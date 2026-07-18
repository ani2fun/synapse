// __SYNAPSE_TRACER__
// Ported from Cortex's JvmTracer HarnessTemplate — the JVM step-tracer harness.
// The client base64-substitutes __SYNAPSE_USER_SOURCE_B64__ (the user's Java source),
// then emits the step trace between the __SYNAPSE_HEAP_BEGIN__ / __SYNAPSE_HEAP_END__ markers.
// Known gaps: Collections/Maps walk as plain objects; lambdas/anonymous classes are not
// statement-instrumented; only concrete-class declared fields are walked.
// JvmTracer Slice 5 harness — full Java surface (helpers, constructors, `this`, instance fields).
import java.io.*;
import java.lang.invoke.MethodHandles;
import java.lang.reflect.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.*;

import javax.tools.*;
import javax.tools.JavaCompiler.CompilationTask;
import javax.tools.JavaFileObject.Kind;

import com.sun.source.tree.*;
import com.sun.source.util.*;

public class Main {
    public static void main(String[] args) {
        try {
            Tracer.run();
        } catch (Throwable t) {
            // Surface failures on stderr so TracedCodeBlock's failed-state UI picks them up.
            t.printStackTrace();
        } finally {
            Tracer.flush(System.out);
        }
    }
}

class Tracer {
    static final int MAX_STEPS = 600;
    static final int MAX_OBJECTS = 400;
    static final int MAX_DEPTH = 60;
    static final int MAX_PAYLOAD = 524288;
    static final int MAX_STRING = 80;

    static List<Step> steps = new ArrayList<>();
    static Deque<Frame> frameStack = new ArrayDeque<>();
    static boolean truncated = false;
    static boolean stopped = false;

    /**
     * Identity → heap id, for the WHOLE trace. Heap ids must be stable across steps: the adapt
     * pipeline compares the root's id BETWEEN steps to decide whether the root rebound to an
     * unrelated structure (a new test case), and diffs ids to mark new/changed nodes. Python's
     * harness gets this free from `id(obj)`; the JVM has no comparable stable handle, so identity
     * is remembered here instead.
     *
     * Numbering per step (the original) renumbers by walk order, so the SAME array changes id the
     * moment the walk order shifts — e.g. entering a method whose `this` is walked first. That
     * reads downstream as "the root became a different object" and split one plain run into three
     * phantom test cases, whose first case showed the array before the algorithm had touched it.
     *
     * This holds strong references for the run's duration, which is bounded by MAX_STEPS in a
     * short-lived sandbox process; an id is never recycled, which is the point.
     */
    static final IdentityHashMap<Object, String> objectIds = new IdentityHashMap<>();
    static int idSequence = 0;

    /** The stable id for `v`, minted on first sight anywhere in the trace. */
    static String idFor(Object v) {
        String id = objectIds.get(v);
        if (id == null) {
            id = String.valueOf(++idSequence);
            objectIds.put(v, id);
        }
        return id;
    }

    /** Dedup state — collapses duplicate "line" events at the same (line, fn) pair. */
    static int lastLineEmitted = Integer.MIN_VALUE;
    static String lastFnEmitted = null;
    static String lastEventEmitted = null;

    static final String USER_SOURCE_B64 = "__SYNAPSE_USER_SOURCE_B64__";

    /** One stack-frame's mutable state: `this` reference (or null for static), and last known locals. */
    static final class Frame {
        final String fn;
        final Object thisRef;
        LinkedHashMap<String, Object> locals;
        int lastLine;
        Frame(String f, Object t) { fn = f; thisRef = t; locals = new LinkedHashMap<>(); }
    }

    /**
     * Push a frame and emit a `"call"` event. Called once at the top of every traced method body
     * (after any explicit `super(...)` / `this(...)` in a constructor — JLS §8.8.7).
     */
    public static void enterFrame(String fn, Object thisRef,
                                  String[] names, Object[] values, int line) {
        if (stopped) return;
        try {
            Frame f = new Frame(fn, thisRef);
            f.lastLine = line;
            int n = Math.min(names.length, values.length);
            for (int i = 0; i < n; i++) f.locals.put(names[i], values[i]);
            frameStack.push(f);
            emitStep(line, "call");
        } catch (Throwable ignored) { }
    }

    /** Emit a `"return"` event then pop the top frame. Fires from each method body's `finally`. */
    public static void exitFrame() {
        if (stopped) return;
        try {
            if (frameStack.isEmpty()) return;
            Frame top = frameStack.peek();
            emitStep(top.lastLine, "return");
            frameStack.pop();
        } catch (Throwable ignored) { }
    }

    /**
     * Per-statement snapshot. Updates the active frame's locals; emits a `"line"` event with dedup
     * against the immediately-previous line event at the same (line, fn) — the duplicate compound +
     * wrapped-body snapshot pair (e.g. `if (cond) return x;`) collapses to one step in the UI.
     */
    public static void snapshot(int line, String[] names, Object[] values) {
        if (stopped) return;
        try {
            if (frameStack.isEmpty()) return;
            Frame top = frameStack.peek();
            top.lastLine = line;
            top.locals.clear();
            int n = Math.min(names.length, values.length);
            for (int i = 0; i < n; i++) top.locals.put(names[i], values[i]);
            if ("line".equals(lastEventEmitted)
                && line == lastLineEmitted
                && top.fn.equals(lastFnEmitted)) {
                return;
            }
            emitStep(line, "line");
        } catch (Throwable ignored) { }
    }

    /**
     * Build a Step from the current frame stack — top frame first (innermost / active), matching
     * the shared `HeapTrace`/`HeapStep.frames` contract (ADR-S029). Each frame's locals are re-walked through the heap so the
     * latest mutable state of any captured object is reflected in this snapshot.
     */
    static void emitStep(int line, String event) {
        if (steps.size() >= MAX_STEPS) { truncated = true; stopped = true; return; }
        // The heap is per step (only what this step reaches); the IDS inside it are not — see
        // `objectIds`. `heap` doubles as the per-step visited set: an entry is inserted BEFORE
        // its contents are walked, so cycles find it and return a plain Ref.
        Map<String, HeapObj> heap = new LinkedHashMap<>();
        List<FrameSnap> snaps = new ArrayList<>();
        for (Frame fr : frameStack) {
            LinkedHashMap<String, Object> locals = new LinkedHashMap<>();
            if (fr.thisRef != null) {
                locals.put("this", visit(fr.thisRef, heap, 0));
            }
            for (Map.Entry<String, Object> e : fr.locals.entrySet()) {
                locals.put(e.getKey(), visit(e.getValue(), heap, 0));
            }
            snaps.add(new FrameSnap(fr.fn, locals));
        }
        steps.add(new Step(line, event, snaps, heap));
        lastLineEmitted = line;
        lastFnEmitted = frameStack.isEmpty() ? null : frameStack.peek().fn;
        lastEventEmitted = event;
    }

    /**
     * Heap walker. Scalars (boolean / character / number / string) serialise inline; native arrays
     * become `{type: "array", items}`; all other objects become `{type: "object", cls, fields}` with
     * declared instance fields expanded via reflection. Synthetic fields (the implicit `this$0` on
     * inner classes) and static fields are skipped — neither is part of the instance state being
     * shown. Ids come from `idFor` and are stable for the whole trace; a cycle (or a second path
     * to the same object) is caught by `heap` already holding the id, since the entry is inserted
     * before its contents are walked. `MAX_OBJECTS` / `MAX_DEPTH` cap walks over pathological
     * graphs — a capped object still gets its stable id, it just isn't expanded.
     */
    static Object visit(Object v, Map<String, HeapObj> heap, int depth) {
        if (v == null) return JNull.INSTANCE;
        if (v instanceof Boolean) return v;
        if (v instanceof Character) return String.valueOf(((Character) v).charValue());
        if (v instanceof Number) return v;
        if (v instanceof String) {
            String s = (String) v;
            return s.length() <= MAX_STRING ? s : s.substring(0, MAX_STRING) + "…";
        }
        String id = idFor(v);
        if (heap.containsKey(id)) return new Ref(id);
        if (heap.size() >= MAX_OBJECTS || depth >= MAX_DEPTH) {
            truncated = true;
            return new Ref(id);
        }
        HeapObj obj = new HeapObj();
        heap.put(id, obj);
        Class<?> cls = v.getClass();
        if (cls.isArray()) {
            obj.type = "array";
            obj.items = new ArrayList<>();
            int len = java.lang.reflect.Array.getLength(v);
            int cap = Math.min(len, MAX_OBJECTS);
            for (int i = 0; i < cap; i++) {
                Object elt = java.lang.reflect.Array.get(v, i);
                obj.items.add(visit(elt, heap, depth + 1));
            }
            if (len > cap) truncated = true;
        } else {
            obj.type = "object";
            obj.cls = userVisibleClassName(cls);
            LinkedHashMap<String, Object> fields = new LinkedHashMap<>();
            for (Field f : cls.getDeclaredFields()) {
                if (f.isSynthetic()) continue;
                if ((f.getModifiers() & Modifier.STATIC) != 0) continue;
                try {
                    f.setAccessible(true);
                    Object fv = f.get(v);
                    fields.put(f.getName(), visit(fv, heap, depth + 1));
                } catch (Throwable ignored) { }
            }
            obj.fields = fields;
        }
        return new Ref(id);
    }

    /**
     * Display name for a class — strip the `_SynUser_` prefix the rewriter added to top-level user
     * types, so the locals panel shows `Node`, not `_SynUser_Node`.
     */
    static String userVisibleClassName(Class<?> cls) {
        String simple = cls.getSimpleName();
        if (simple.startsWith(Rewriter.USER_CLASS_PREFIX)) {
            return simple.substring(Rewriter.USER_CLASS_PREFIX.length());
        }
        return simple;
    }

    /**
     * Drive the whole rewrite-compile-run cycle. The rewriter renames all top-level user types to
     * `_SynUser_<orig>` so they don't collide with harness names; the compiled bytes are defined into
     * Tracer's own loader so `Tracer.snapshot` / `Tracer.enterFrame` resolve cross-class without an
     * `IllegalAccessError` (the Slice 4 cross-loader bug).
     */
    static void run() throws Throwable {
        String source = new String(Base64.getDecoder().decode(USER_SOURCE_B64), StandardCharsets.UTF_8);
        Rewriter.Result r = Rewriter.rewrite(source);
        Map<String, byte[]> classes = compileToBytes(r.rewritten, r.entrypointClass);
        byte[] entrypointBytes = classes.get(r.entrypointClass);
        if (entrypointBytes == null) {
            throw new RuntimeException(
                "Compiled bytecode missing for entrypoint class " + r.entrypointClass
            );
        }
        MethodHandles.Lookup lookup = MethodHandles.lookup();
        Class<?> userMain = lookup.defineClass(entrypointBytes);
        for (Map.Entry<String, byte[]> e : classes.entrySet()) {
            if (!e.getKey().equals(r.entrypointClass)) {
                lookup.defineClass(e.getValue());
            }
        }
        Method mainMethod = userMain.getMethod("main", String[].class);
        mainMethod.invoke(null, (Object) new String[0]);
    }

    /**
     * Compile the rewritten user source to in-memory bytecode. The source file is named after the
     * entrypoint class so Java's "public class X must live in X.java" rule is satisfied. The host JVM's
     * class path is forwarded so the rewritten user code can resolve `Tracer` by simple name.
     */
    static Map<String, byte[]> compileToBytes(String source, String entrypointClass) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        if (compiler == null) {
            throw new RuntimeException("No system Java compiler — jdk.compiler missing on this backend.");
        }
        StandardJavaFileManager std =
            compiler.getStandardFileManager(null, null, StandardCharsets.UTF_8);
        InMemoryFileManager fm = new InMemoryFileManager(std);
        InMemorySource src = new InMemorySource(entrypointClass, source);
        DiagnosticCollector<JavaFileObject> diags = new DiagnosticCollector<>();
        List<String> options = Arrays.asList(
            "-classpath", System.getProperty("java.class.path", ".")
        );
        CompilationTask task = compiler.getTask(
            null, fm, diags, options, null, Collections.singletonList(src)
        );
        Boolean ok = task.call();
        if (ok == null || !ok.booleanValue()) {
            StringBuilder msg = new StringBuilder("Trace harness failed to compile user source:\n");
            for (Diagnostic<? extends JavaFileObject> d : diags.getDiagnostics()) {
                msg.append(d.toString()).append('\n');
            }
            throw new RuntimeException(msg.toString());
        }
        return fm.bytesByName();
    }

    static void flush(PrintStream out) {
        out.print("\n__SYNAPSE_HEAP_BEGIN__");
        out.print(buildJson());
        out.print("__SYNAPSE_HEAP_END__\n");
    }

    /** Quarter-drop truncation loop matching the Python harness's payload-size guard.
     *  Drops the LAST quarter so the algorithm's setup + first iterations survive — the
     *  "showing the first part of the run" banner the modal renders depends on this. */
    static String buildJson() {
        while (true) {
            StringBuilder sb = new StringBuilder();
            writeTrace(sb);
            if (sb.length() <= MAX_PAYLOAD || steps.size() <= 1) return sb.toString();
            int drop = steps.size() / 4 + 1;
            steps = new ArrayList<>(steps.subList(0, steps.size() - drop));
            truncated = true;
        }
    }

    static void writeTrace(StringBuilder sb) {
        sb.append("{\"steps\":[");
        for (int i = 0; i < steps.size(); i++) {
            if (i > 0) sb.append(',');
            writeStep(sb, steps.get(i));
        }
        sb.append("],\"truncated\":").append(truncated).append('}');
    }

    static void writeStep(StringBuilder sb, Step s) {
        sb.append("{\"line\":").append(s.line);
        sb.append(",\"event\":\"").append(s.event).append("\",\"frames\":[");
        for (int i = 0; i < s.frames.size(); i++) {
            if (i > 0) sb.append(',');
            FrameSnap f = s.frames.get(i);
            sb.append("{\"fn\":");
            writeStr(sb, f.fn);
            sb.append(",\"locals\":{");
            boolean first = true;
            for (Map.Entry<String, Object> e : f.locals.entrySet()) {
                if (!first) sb.append(',');
                first = false;
                writeStr(sb, e.getKey());
                sb.append(':');
                writeVal(sb, e.getValue());
            }
            sb.append("}}");
        }
        sb.append("],\"heap\":{");
        boolean first = true;
        for (Map.Entry<String, HeapObj> e : s.heap.entrySet()) {
            if (!first) sb.append(',');
            first = false;
            writeStr(sb, e.getKey());
            sb.append(':');
            writeHeap(sb, e.getValue());
        }
        sb.append("}}");
    }

    static void writeHeap(StringBuilder sb, HeapObj o) {
        sb.append("{\"type\":\"").append(o.type).append('"');
        if (o.items != null) {
            sb.append(",\"items\":[");
            for (int i = 0; i < o.items.size(); i++) {
                if (i > 0) sb.append(',');
                writeVal(sb, o.items.get(i));
            }
            sb.append(']');
        }
        if (o.cls != null) {
            sb.append(",\"cls\":");
            writeStr(sb, o.cls);
            sb.append(",\"fields\":{");
            if (o.fields != null) {
                boolean first = true;
                for (Map.Entry<String, Object> e : o.fields.entrySet()) {
                    if (!first) sb.append(',');
                    first = false;
                    writeStr(sb, e.getKey());
                    sb.append(':');
                    writeVal(sb, e.getValue());
                }
            }
            sb.append('}');
        }
        sb.append('}');
    }

    static void writeVal(StringBuilder sb, Object v) {
        if (v == JNull.INSTANCE || v == null) {
            sb.append("null");
        } else if (v instanceof Ref) {
            sb.append("{\"ref\":\"").append(((Ref) v).id).append("\"}");
        } else if (v instanceof Boolean) {
            sb.append(((Boolean) v).booleanValue() ? "true" : "false");
        } else if (v instanceof Number) {
            Number n = (Number) v;
            if (n instanceof Double || n instanceof Float) {
                double d = n.doubleValue();
                if (Double.isNaN(d) || Double.isInfinite(d)) writeStr(sb, n.toString());
                else sb.append(n.toString());
            } else {
                sb.append(n.toString());
            }
        } else if (v instanceof String) {
            writeStr(sb, (String) v);
        } else {
            writeStr(sb, String.valueOf(v));
        }
    }

    static void writeStr(StringBuilder sb, String s) {
        sb.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '"') sb.append("\\\"");
            else if (c == '\\') sb.append("\\\\");
            else if (c == '\n') sb.append("\\n");
            else if (c == '\r') sb.append("\\r");
            else if (c == '\t') sb.append("\\t");
            else if (c < 0x20) sb.append(String.format("\\u%04x", (int) c));
            else sb.append(c);
        }
        sb.append('"');
    }

    static final class Step {
        int line; String event; List<FrameSnap> frames; Map<String, HeapObj> heap;
        Step(int l, String e, List<FrameSnap> f, Map<String, HeapObj> h) {
            line = l; event = e; frames = f; heap = h;
        }
    }
    static final class FrameSnap {
        String fn; LinkedHashMap<String, Object> locals;
        FrameSnap(String n, LinkedHashMap<String, Object> l) { fn = n; locals = l; }
    }
    static final class HeapObj {
        String type; String cls; List<Object> items; Map<String, Object> fields;
    }
    static final class Ref { final String id; Ref(String i) { id = i; } }
    enum JNull { INSTANCE }
}

class InMemorySource extends SimpleJavaFileObject {
    private final String code;
    InMemorySource(String className, String code) {
        super(URI.create("mem:///" + className + ".java"), Kind.SOURCE);
        this.code = code;
    }
    @Override public CharSequence getCharContent(boolean ignore) { return code; }
}

class InMemoryClass extends SimpleJavaFileObject {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    InMemoryClass(String binaryName) {
        super(URI.create("mem:///" + binaryName.replace('.', '/') + ".class"), Kind.CLASS);
    }
    @Override public OutputStream openOutputStream() { return out; }
    byte[] bytes() { return out.toByteArray(); }
}

class InMemoryFileManager extends ForwardingJavaFileManager<JavaFileManager> {
    private final Map<String, InMemoryClass> classes = new HashMap<>();

    InMemoryFileManager(JavaFileManager fm) { super(fm); }

    @Override
    public JavaFileObject getJavaFileForOutput(Location loc,
                                               String className,
                                               Kind kind,
                                               FileObject sibling) throws IOException {
        if (kind == Kind.CLASS) {
            InMemoryClass c = new InMemoryClass(className);
            classes.put(className, c);
            return c;
        }
        return super.getJavaFileForOutput(loc, className, kind, sibling);
    }

    Map<String, byte[]> bytesByName() {
        Map<String, byte[]> out = new HashMap<>();
        for (Map.Entry<String, InMemoryClass> e : classes.entrySet()) {
            out.put(e.getKey(), e.getValue().bytes());
        }
        return out;
    }
}

/**
 * Walks the parsed user source and emits text edits. Three kinds:
 *
 *   - **Class rename** — every top-level user type declaration (class / interface / enum / record) is
 *     prefixed with `_SynUser_` so user names can't collide with harness names like `Tracer`,
 *     `InMemoryFileManager`, …. A first pass over the compilation unit's `getTypeDecls()` populates
 *     `userClassNames`; the main scan then emits a rename edit at every `IdentifierTree` whose name
 *     matches.
 *   - **Snapshot splice** — `Tracer.snapshot(line, names, values)` precedes every statement in every
 *     method body. Unbraced `if/while/for/do` bodies are wrapped in `{}` first so the splice lands in
 *     a real block.
 *   - **Frame-stack wrapping** — each method body becomes
 *     `{ Tracer.enterFrame(...); try { … } finally { Tracer.exitFrame(); } }`. For constructors with
 *     an explicit `super(...)` / `this(...)` first statement, the enterFrame insertion shifts to
 *     immediately after that statement to honour JLS §8.8.7.
 *
 * Lambdas and anonymous class bodies are NOT instrumented (deferred — see plan §Slice 5). Identifier
 * renames continue to apply inside them so references resolve.
 *
 * Edits are sorted descending by `(offset, tiebreak)` then applied via `StringBuilder.insert`. Higher
 * tiebreak = applied first = ends up further RIGHT at a shared offset. Tiebreak ladder (Slice 5):
 *     TB_FINALLY (2) > TB_SNAPSHOT (1) > TB_OPEN_BRACE/TB_DEFAULT (0) > TB_ENTER_FRAME (-1)
 * so at any one offset the layout is `enterFrame ; openBrace ; snapshot ; ... ; finally`.
 */
class Rewriter {

    static final int TB_FINALLY = 2;
    static final int TB_SNAPSHOT = 1;
    static final int TB_OPEN_BRACE = 0;
    static final int TB_DEFAULT = 0;
    static final int TB_ENTER_FRAME = -1;

    /** Prefix used to rename user types so they can't shadow harness class names. */
    static final String USER_CLASS_PREFIX = "_SynUser_";

    static final class Result {
        String rewritten;
        String entrypointClass;
    }

    static final class Edit {
        final int offset;
        final int tiebreak;
        final String text;
        Edit(int o, int t, String s) { offset = o; tiebreak = t; text = s; }
    }

    static Result rewrite(String source) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        if (compiler == null) {
            throw new RuntimeException("No system Java compiler — jdk.compiler missing on this backend.");
        }
        StandardJavaFileManager fm =
            compiler.getStandardFileManager(null, null, StandardCharsets.UTF_8);
        InMemorySource src = new InMemorySource("UserSourceParse", source);
        DiagnosticCollector<JavaFileObject> diags = new DiagnosticCollector<>();
        JavacTask task = (JavacTask) compiler.getTask(
            null, fm, diags, null, null, Collections.singletonList(src)
        );
        Iterator<? extends CompilationUnitTree> it = task.parse().iterator();
        if (!it.hasNext()) {
            throw new RuntimeException("Tracer parse pass produced no compilation unit.");
        }
        CompilationUnitTree unit = it.next();
        SourcePositions positions = Trees.instance(task).getSourcePositions();

        Walker walker = new Walker(source, unit, positions);
        walker.collectTopLevelTypes();
        walker.scan(unit, null);

        if (walker.entrypointClass == null) {
            throw new RuntimeException(
                "Tracer needs a `public static void main(String[] args)` method to instrument."
            );
        }

        walker.edits.sort((a, b) -> {
            int c = Integer.compare(b.offset, a.offset);
            if (c != 0) return c;
            return Integer.compare(b.tiebreak, a.tiebreak);
        });
        StringBuilder sb = new StringBuilder(source);
        for (Edit e : walker.edits) {
            sb.insert(e.offset, e.text);
        }
        Result r = new Result();
        r.rewritten = sb.toString();
        r.entrypointClass = walker.entrypointClass;
        return r;
    }

    static final class Walker extends TreeScanner<Void, Void> {
        final String source;
        final CompilationUnitTree unit;
        final SourcePositions positions;
        final List<Edit> edits = new ArrayList<>();
        /** Names of every top-level user type — drives identifier renaming in visitIdentifier. */
        final Set<String> userClassNames = new HashSet<>();
        /** Source offsets of top-level type-name tokens — used to suppress double-rename of decls. */
        final Set<Integer> topLevelNamePositions = new HashSet<>();
        final Deque<List<String>> scopes = new ArrayDeque<>();
        /** Offsets already carrying a snapshot — so a multi-declarator's sibling decls emit only one. */
        final Set<Integer> snapshotOffsets = new HashSet<>();
        String currentFn = null;
        String currentClass = null;
        /** Renamed name of the class declaring `main(String[])` — what `Class.forName` resolves to. */
        String entrypointClass = null;
        /** Depth of enclosing lambda / anonymous-class bodies; non-zero ⇒ skip instrumentation. */
        int lambdaOrAnonDepth = 0;

        Walker(String src, CompilationUnitTree u, SourcePositions p) {
            source = src; unit = u; positions = p;
        }

        /**
         * First-pass over the compilation unit's top-level declarations: record every user type name
         * and emit a rename edit at its name token. Run BEFORE the main scan so `userClassNames` is
         * fully populated when `visitIdentifier` starts firing.
         */
        void collectTopLevelTypes() {
            for (Tree decl : unit.getTypeDecls()) {
                if (!(decl instanceof ClassTree)) continue;
                ClassTree ct = (ClassTree) decl;
                String name = ct.getSimpleName().toString();
                if (name.isEmpty()) continue;
                int pos = findClassNamePosition(ct, name);
                if (pos < 0) continue;
                userClassNames.add(name);
                topLevelNamePositions.add(pos);
                edits.add(new Edit(pos, TB_DEFAULT, USER_CLASS_PREFIX));
            }
        }

        @Override
        public Void visitClass(ClassTree node, Void p) {
            String prev = currentClass;
            currentClass = node.getSimpleName().toString();
            Void r = super.visitClass(node, p);
            currentClass = prev;
            return r;
        }

        /** Skip imports — qualified import-path identifiers must never be renamed. */
        @Override
        public Void visitImport(ImportTree node, Void p) { return null; }

        @Override
        public Void visitIdentifier(IdentifierTree node, Void p) {
            String name = node.getName().toString();
            if (userClassNames.contains(name)) {
                long pos = positions.getStartPosition(unit, node);
                if (pos >= 0 && !topLevelNamePositions.contains((int) pos)) {
                    edits.add(new Edit((int) pos, TB_DEFAULT, USER_CLASS_PREFIX));
                }
            }
            return super.visitIdentifier(node, p);
        }

        /** Lambdas: recurse for identifier renames but don't instrument statements. */
        @Override
        public Void visitLambdaExpression(LambdaExpressionTree node, Void p) {
            lambdaOrAnonDepth++;
            try {
                return super.visitLambdaExpression(node, p);
            } finally {
                lambdaOrAnonDepth--;
            }
        }

        /** Anonymous-class bodies: rename refs inside, but skip statement instrumentation. */
        @Override
        public Void visitNewClass(NewClassTree node, Void p) {
            scan(node.getEnclosingExpression(), p);
            scan(node.getIdentifier(), p);
            for (Tree arg : node.getTypeArguments()) scan(arg, p);
            for (Tree arg : node.getArguments()) scan(arg, p);
            if (node.getClassBody() != null) {
                lambdaOrAnonDepth++;
                try {
                    scan(node.getClassBody(), p);
                } finally {
                    lambdaOrAnonDepth--;
                }
            }
            return null;
        }

        @Override
        public Void visitMethod(MethodTree node, Void p) {
            // Scan non-body parts so parameter / return / throws types get renamed.
            if (node.getModifiers() != null) scan(node.getModifiers(), p);
            if (node.getReturnType() != null) scan(node.getReturnType(), p);
            if (node.getTypeParameters() != null) {
                for (Tree tp : node.getTypeParameters()) scan(tp, p);
            }
            if (node.getParameters() != null) {
                for (VariableTree pv : node.getParameters()) scan(pv, p);
            }
            if (node.getThrows() != null) {
                for (Tree t : node.getThrows()) scan(t, p);
            }
            if (node.getDefaultValue() != null) scan(node.getDefaultValue(), p);

            BlockTree body = node.getBody();
            if (body == null) return null;

            if (lambdaOrAnonDepth > 0) {
                // Inside a lambda / anonymous class — recurse for identifier renames only.
                scan(body, p);
                return null;
            }

            String name = node.getName().toString();
            // `Modifier` is ambiguous — `java.lang.reflect.Modifier` (used in the heap walker for
            // `Field.getModifiers() & Modifier.STATIC`) wins the wildcard-import contest. The AST
            // flag is from a different package, so qualify it explicitly.
            boolean isStatic = node.getModifiers().getFlags()
                .contains(javax.lang.model.element.Modifier.STATIC);

            // Entrypoint: the first `public static void main(String[] args)` in a top-level class.
            if ("main".equals(name)
                && isStatic
                && node.getParameters().size() == 1
                && entrypointClass == null
                && currentClass != null
                && userClassNames.contains(currentClass)) {
                entrypointClass = USER_CLASS_PREFIX + currentClass;
            }

            String prevFn = currentFn;
            currentFn = name;
            scopes.push(new ArrayList<>());
            for (VariableTree v : node.getParameters()) {
                scopes.peek().add(v.getName().toString());
            }
            emitMethodEntryExit(node, body, isStatic);
            scan(body, p);
            scopes.pop();
            currentFn = prevFn;
            return null;
        }

        /**
         * Emit `Tracer.enterFrame(...);try{` after the body's `{` and `}finally{Tracer.exitFrame();}`
         * before the body's `}`. For a constructor whose first statement is `super(...)` / `this(...)`
         * the enterFrame shifts to AFTER that statement (JLS §8.8.7 forbids any statement before it).
         */
        void emitMethodEntryExit(MethodTree method, BlockTree body, boolean isStatic) {
            long bodyStart = positions.getStartPosition(unit, body);
            long bodyEnd = positions.getEndPosition(unit, body);
            if (bodyStart < 0 || bodyEnd < 0) return;

            int enterAt = (int) bodyStart + 1;
            String name = method.getName().toString();
            boolean isCtor = "<init>".equals(name);
            if (isCtor && !body.getStatements().isEmpty()) {
                StatementTree first = body.getStatements().get(0);
                if (isSuperOrThisCall(first)) {
                    long firstEnd = positions.getEndPosition(unit, first);
                    if (firstEnd >= 0) enterAt = (int) firstEnd;
                }
            }
            int finallyAt = (int) bodyEnd - 1;
            if (finallyAt < enterAt) return;

            long line = unit.getLineMap().getLineNumber(bodyStart);

            StringBuilder enter = new StringBuilder();
            enter.append("Tracer.enterFrame(\"").append(name).append("\",");
            enter.append(isStatic ? "null" : "this");
            enter.append(",new String[]{");
            boolean first = true;
            for (VariableTree pv : method.getParameters()) {
                if (!first) enter.append(',');
                first = false;
                enter.append('"').append(pv.getName().toString()).append('"');
            }
            enter.append("},new Object[]{");
            first = true;
            for (VariableTree pv : method.getParameters()) {
                if (!first) enter.append(',');
                first = false;
                enter.append(pv.getName().toString());
            }
            enter.append("},").append(line).append(");try{");

            edits.add(new Edit(enterAt, TB_ENTER_FRAME, enter.toString()));
            edits.add(new Edit(finallyAt, TB_FINALLY, "}finally{Tracer.exitFrame();}"));
        }

        /** True iff `stmt` is an `ExpressionStatement` wrapping a `super(...)` or `this(...)` call. */
        static boolean isSuperOrThisCall(StatementTree stmt) {
            if (!(stmt instanceof ExpressionStatementTree)) return false;
            ExpressionTree expr = ((ExpressionStatementTree) stmt).getExpression();
            if (!(expr instanceof MethodInvocationTree)) return false;
            ExpressionTree select = ((MethodInvocationTree) expr).getMethodSelect();
            if (!(select instanceof IdentifierTree)) return false;
            String n = ((IdentifierTree) select).getName().toString();
            return "super".equals(n) || "this".equals(n);
        }

        /**
         * Locate the start offset of a top-level type's name token in the source. Scans forward from
         * the declaration's start position for whichever of `class` / `interface` / `enum` / `record`
         * appears earliest, then for the next identifier matching `name`. Returns -1 if not found —
         * malformed declarations or unsupported syntactic edge cases simply skip rename.
         */
        int findClassNamePosition(ClassTree node, String name) {
            long declStart = positions.getStartPosition(unit, node);
            if (declStart < 0) return -1;
            int kw = -1;
            int kwLen = 0;
            for (String keyword : new String[]{"class", "interface", "enum", "record"}) {
                int k = findKeyword(source, (int) declStart, keyword);
                if (k >= 0 && (kw < 0 || k < kw)) {
                    kw = k;
                    kwLen = keyword.length();
                }
            }
            if (kw < 0) return -1;
            int idx = kw + kwLen;
            while (idx < source.length() && Character.isWhitespace(source.charAt(idx))) idx++;
            if (idx + name.length() > source.length()) return -1;
            if (!source.startsWith(name, idx)) return -1;
            return idx;
        }

        static int findKeyword(String src, int from, String kw) {
            int len = kw.length();
            int i = Math.max(0, from);
            while (i + len <= src.length()) {
                if (src.startsWith(kw, i)) {
                    boolean before = (i == 0) || !Character.isJavaIdentifierPart(src.charAt(i - 1));
                    boolean after = (i + len >= src.length())
                        || !Character.isJavaIdentifierPart(src.charAt(i + len));
                    if (before && after) return i;
                }
                i++;
            }
            return -1;
        }

        @Override
        public Void visitBlock(BlockTree node, Void p) {
            scopes.push(new ArrayList<>());
            for (StatementTree stmt : node.getStatements()) {
                addSnapshotEdit(stmt);
                scan(stmt, p);
                if (stmt instanceof VariableTree) {
                    VariableTree v = (VariableTree) stmt;
                    if (v.getInitializer() != null) scopes.peek().add(v.getName().toString());
                }
            }
            scopes.pop();
            return null;
        }

        /** Body of a compound construct (if/while/for/do): wrap with {} if unbraced; emit snapshot. */
        void wrapAndSnap(StatementTree stmt, Void p) {
            if (stmt == null) return;
            if (stmt instanceof BlockTree) {
                scan(stmt, p);
                return;
            }
            long start = positions.getStartPosition(unit, stmt);
            long end = positions.getEndPosition(unit, stmt);
            if (start < 0 || end < 0) {
                scan(stmt, p);
                return;
            }
            edits.add(new Edit((int) start, TB_OPEN_BRACE, "{"));
            edits.add(new Edit((int) end, TB_DEFAULT, "}"));
            addSnapshotEdit(stmt);
            scopes.push(new ArrayList<>());
            scan(stmt, p);
            if (stmt instanceof VariableTree) {
                VariableTree v = (VariableTree) stmt;
                if (v.getInitializer() != null) scopes.peek().add(v.getName().toString());
            }
            scopes.pop();
        }

        @Override
        public Void visitIf(IfTree node, Void p) {
            scan(node.getCondition(), p);
            wrapAndSnap(node.getThenStatement(), p);
            if (node.getElseStatement() != null) wrapAndSnap(node.getElseStatement(), p);
            return null;
        }

        @Override
        public Void visitWhileLoop(WhileLoopTree node, Void p) {
            scan(node.getCondition(), p);
            wrapAndSnap(node.getStatement(), p);
            afterLoopSnap(node);
            return null;
        }

        @Override
        public Void visitDoWhileLoop(DoWhileLoopTree node, Void p) {
            wrapAndSnap(node.getStatement(), p);
            scan(node.getCondition(), p);
            afterLoopSnap(node);
            return null;
        }

        @Override
        public Void visitForLoop(ForLoopTree node, Void p) {
            scopes.push(new ArrayList<>());
            for (StatementTree init : node.getInitializer()) {
                scan(init, p);
                if (init instanceof VariableTree) {
                    VariableTree v = (VariableTree) init;
                    if (v.getInitializer() != null) scopes.peek().add(v.getName().toString());
                }
            }
            scan(node.getCondition(), p);
            for (ExpressionStatementTree upd : node.getUpdate()) scan(upd, p);
            wrapAndSnap(node.getStatement(), p);
            scopes.pop();
            afterLoopSnap(node); // after pop → the for's own index vars are (correctly) out of scope
            return null;
        }

        @Override
        public Void visitEnhancedForLoop(EnhancedForLoopTree node, Void p) {
            scopes.push(new ArrayList<>());
            scan(node.getExpression(), p);
            scopes.peek().add(node.getVariable().getName().toString());
            wrapAndSnap(node.getStatement(), p);
            scopes.pop();
            afterLoopSnap(node); // after pop → the loop variable is (correctly) out of scope
            return null;
        }

        @Override
        public Void visitLabeledStatement(LabeledStatementTree node, Void p) {
            scan(node.getStatement(), p);
            return null;
        }

        void addSnapshotEdit(StatementTree stmt) {
            if (currentFn == null) return;
            if (stmt == null) return;
            if (stmt instanceof BlockTree) return;
            if (stmt.getKind() == Tree.Kind.EMPTY_STATEMENT) return;
            long start = positions.getStartPosition(unit, stmt);
            if (start < 0) return;
            // A multi-declarator (`int a = 1, b = 2;`) parses as sibling VariableTrees sharing this one
            // start offset; keep only the first snapshot (taken before any sibling is in scope) — a later
            // sibling's snapshot would reference a name not yet declared at this splice point.
            if (!snapshotOffsets.add((int) start)) return;
            long line = unit.getLineMap().getLineNumber(start);
            edits.add(new Edit((int) start, TB_SNAPSHOT, snapshotCall(line)));
        }

        /** `Tracer.snapshot(line, names, values);` over every in-scope local (shared by statement + loop-exit). */
        String snapshotCall(long line) {
            StringBuilder snap = new StringBuilder();
            snap.append("Tracer.snapshot(").append(line).append(",new String[]{");
            StringBuilder vals = new StringBuilder();
            boolean first = true;
            for (List<String> scope : scopes) {
                for (String n : scope) {
                    if (!first) { snap.append(','); vals.append(','); }
                    first = false;
                    snap.append('"').append(n).append('"');
                    vals.append(n);
                }
            }
            return snap.append("},new Object[]{").append(vals).append("});").toString();
        }

        /**
         * A loop instruments only its body statements, so nothing captures the **exit** — the moment its
         * condition goes false (e.g. two pointers meeting). Python's tracer records that as the while-line
         * re-check; here we splice one snapshot right AFTER the loop, at the loop's line, over the enclosing
         * scope (the loop's own variables are already popped, so correctly excluded — all remaining names are
         * definitely-assigned, so this never breaks compilation). It captures the true final state, and also
         * refreshes the frame's locals so the method's `"return"` event is consistent rather than stale. A
         * duplicate terminal frame (return == this snapshot) collapses downstream via StepFlow coalesce.
         */
        void afterLoopSnap(Tree loopNode) {
            if (currentFn == null) return;
            long start = positions.getStartPosition(unit, loopNode);
            long end = positions.getEndPosition(unit, loopNode);
            if (start < 0 || end < 0) return;
            edits.add(new Edit((int) end, TB_SNAPSHOT, snapshotCall(unit.getLineMap().getLineNumber(start))));
        }
    }
}
