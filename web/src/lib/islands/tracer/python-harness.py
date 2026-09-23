# ──────────────────────────────────────────────────────────────────
# SYNAPSE PYTHON TRACER HARNESS (step 28)
# ──────────────────────────────────────────────────────────────────
# A sys.settrace harness ported from the Cortex oracle. The user's source is
# base64-embedded (python.ts substitutes the placeholder below), compiled
# under the filename "<traced>", and executed while a trace function snapshots
# the call stack + heap on every line/call/return event in a user frame. The
# trace JSON is printed between the __SYNAPSE_HEAP_* markers AFTER the program's
# own output, so the client can split program stdout from the trace.
#
# Budgets keep a teaching-size run bounded: 600 steps, 400 objects, depth 60,
# 512 KB payload (drop the LAST quarter of steps repeatedly if over — keep the
# setup + early iterations). Names are `_syn_*` so they're filtered out of the
# user's locals.
#
# A RUN THAT ENDS BADLY still reports why: an uncaught exception, or a source that
# never compiled at all, is the only useful thing such a run has to say. The steps
# recorded while the exception unwinds are a `return` per frame at the line that
# raised, so they are trimmed and the story ends on the line that actually broke.
# Every step also records how many bytes the program had PRINTED by then, so output
# can arrive as the reader steps rather than all of it at step 0.
#
# What a reader never wrote is not stepped: a CLASS BODY runs once, at the `class`
# line, and its methods are traced when they are called. What identifies a thing is
# kept: a function is its signature, and a class the reader defined keeps its own
# members, so the canvas can say which box is which. And on 3.12+ a comprehension
# at module scope is INLINED (PEP 709): mid-loop, the module frame's f_locals holds
# only the comprehension's own variables, so the globals are read from f_globals and
# the comprehension's variables travel separately, as the frame's "comp".
#
# INPUT is served from stdin and RECORDED. The sandbox runs a program once, with
# stdin fixed up front, so a reader cannot type into a running program — but they
# can be asked. `input()` is replaced with a wrapper that logs every value it
# serves and, when stdin runs dry, sets `waiting` and stops the program THERE
# rather than raising EOFError. The client shows a prompt at that step, and a
# re-run with one more line continues the story. Which makes `inputs` the
# contract that matters: it is what the client replays, so a run that served
# three values must report exactly those three, in order — each as
# {"v": value, "at": the step that read it}, because the client walks the steps
# and has to know which values the program had reached by the one on screen.
import sys, json, base64, math, types

_syn_source = base64.b64decode("__SYNAPSE_USER_SOURCE_B64__").decode("utf-8")
_syn_steps = []
_syn_truncated = [False]
_syn_inputs = []
_syn_waiting = [False]
_syn_prompt = [""]
# The exception that ENDED the run, and the step it was raised at. Separate, because they are
# learned at different moments: the tracer sees every raise (caught ones included) and only the
# `except` around exec() knows which one the program failed to survive.
_syn_error = [None]
_syn_raised_at = [None]
_syn_step_limit = 600
_syn_max_objects = 400
_syn_max_depth = 60
_syn_max_payload = 512 * 1024

# CPython code-object flags, stable across versions. A class body and the module are the only
# code objects that are not OPTIMIZED; functions, methods, lambdas and generators all are.
_SYN_CO_OPTIMIZED = 0x0001
_SYN_CO_VARARGS = 0x0004
_SYN_CO_VARKEYWORDS = 0x0008

# Modules whose objects are stdlib/library internals, not user data — render their
# instances opaque (no field recursion) so importing `deque`/`Optional` doesn't drag
# the metaclass tree into every snapshot.
_syn_opaque_modules = frozenset((
    "typing", "_collections_abc", "collections.abc", "abc",
    "_typeshed", "_collections", "_weakrefset", "weakref",
))

# Injected into the traced globals, so they surface as module-frame locals unless named
# here — a reader who never wrote `input` should not see it among their variables.
_syn_hidden = frozenset(("input",))

class _SynStdout:
    """A tee that COUNTS. Every step records how many bytes the program had printed by the time
    it ran, so the client can fill an output box AS the reader steps instead of showing the whole
    run's output at step 0 — which tells them the answer before the program has worked it out.

    Bytes, not characters: the client slices the UTF-8 it decoded, and a `π` is one character of
    two bytes."""

    def __init__(self, inner):
        self._inner = inner
        self.written = 0

    def write(self, text):
        self.written += len(text.encode("utf-8", "replace"))
        return self._inner.write(text)

    def flush(self):
        self._inner.flush()

    def __getattr__(self, name):
        return getattr(self._inner, name)

_syn_stdout = _SynStdout(sys.stdout)
sys.stdout = _syn_stdout

def _syn_is_opaque(v):
    if isinstance(v, type): return True
    if isinstance(v, types.ModuleType): return True
    if isinstance(v, (types.FunctionType, types.BuiltinFunctionType,
                       types.MethodType, types.BuiltinMethodType,
                       types.MethodWrapperType, types.WrapperDescriptorType,
                       types.MethodDescriptorType, types.GetSetDescriptorType,
                       types.MemberDescriptorType)):
        return True
    mod = getattr(type(v), "__module__", "")
    return mod in _syn_opaque_modules

def _syn_signature(fn):
    """`name(a, b, *rest, k, **kw)` — the parameter NAMES in declaration order, which is what a
    reader needs to tell one function box from the next. Annotations and defaults are left out:
    the source beside the canvas already says them."""
    code = fn.__code__
    names = code.co_varnames
    positional = code.co_argcount
    keyword_only = code.co_kwonlyargcount
    params = list(names[:positional])
    # co_varnames holds the positional names, then the keyword-only ones, then *args, then **kw.
    at = positional + keyword_only
    if code.co_flags & _SYN_CO_VARARGS:
        params.append("*" + names[at])
        at += 1
    elif keyword_only:
        params.append("*")
    params.extend(names[positional:positional + keyword_only])
    if code.co_flags & _SYN_CO_VARKEYWORDS:
        params.append("**" + names[at])
    return "%s(%s)" % (fn.__name__, ", ".join(params))

def _syn_scalar(v):
    if v is None or isinstance(v, bool) or isinstance(v, int):
        return (True, v)
    if isinstance(v, float):
        return (True, v if math.isfinite(v) else repr(v))
    if isinstance(v, str):
        return (True, v if len(v) <= 80 else v[:80] + "…")
    return (False, None)

# Snapshot the call stack (a list of (fn_name, locals_items, comp_items), innermost first) into the
# frames/heap shape. One shared heap so an object referenced from two frames is one node.
def _syn_snapshot(frame_specs):
    heap = {}
    def visit(v, depth):
        is_s, sv = _syn_scalar(v)
        if is_s:
            return sv
        oid = str(id(v))
        if oid in heap:
            return {"ref": oid}
        if len(heap) >= _syn_max_objects or depth >= _syn_max_depth:
            _syn_truncated[0] = True
            return {"ref": oid}
        if isinstance(v, types.FunctionType):
            heap[oid] = {"type": "function", "sig": _syn_signature(v)}
            return {"ref": oid}
        if isinstance(v, type) and getattr(v, "__module__", None) == "__main__":
            # A class the READER defined keeps its own members — the methods they wrote, and any
            # class attributes. A library class stays opaque: its members are the library's.
            heap[oid] = None
            members = {}
            for mk, mv in list(vars(v).items()):
                if not isinstance(mk, str):
                    continue
                if isinstance(mv, (staticmethod, classmethod)):
                    mv = mv.__func__
                # Every method the reader wrote, `__init__` included. The other dunders are the
                # ones Python adds itself — `__module__`, `__qualname__`, `__dict__`, `__doc__` —
                # bookkeeping about the class rather than members of it.
                if mk.startswith("__") and not isinstance(mv, types.FunctionType):
                    continue
                members[mk] = visit(mv, depth + 1)
            heap[oid] = {"type": "class", "name": v.__name__, "members": members}
            return {"ref": oid}
        if _syn_is_opaque(v):
            # A CLASS is named for itself, not for its metaclass: `type(Solution).__name__` is
            # "type", which tells a reader nothing about the box their variable points at.
            cls = (v.__name__ + " class") if isinstance(v, type) else type(v).__name__
            heap[oid] = {"type": "object", "cls": cls, "fields": {}}
            return {"ref": oid}
        heap[oid] = None
        if isinstance(v, (list, tuple)):
            kind = "list" if isinstance(v, list) else "tuple"
            heap[oid] = {"type": kind,
                         "items": [visit(x, depth + 1) for x in list(v)[:_syn_max_objects]]}
        elif isinstance(v, dict):
            entries = []
            for dk, dv in list(v.items())[:_syn_max_objects]:
                entries.append([visit(dk, depth + 1), visit(dv, depth + 1)])
            heap[oid] = {"type": "dict", "entries": entries}
        else:
            d = getattr(v, "__dict__", None)
            if d is None:
                d = {}
                for sl in (getattr(type(v), "__slots__", ()) or ()):
                    if isinstance(sl, str) and hasattr(v, sl):
                        d[sl] = getattr(v, sl)
            fields = {}
            for fk, fv in list(d.items()):
                if isinstance(fk, str) and not fk.startswith("_syn_"):
                    fields[fk] = visit(fv, depth + 1)
            heap[oid] = {"type": "object", "cls": type(v).__name__, "fields": fields}
        return {"ref": oid}
    def names(items):
        out = {}
        for k, v in items:
            if isinstance(k, str) and not k.startswith("_syn_") and not k.startswith("__") \
                    and k not in _syn_hidden:
                out[k] = visit(v, 0)
        return out
    frames_out = []
    for fn_name, items, comp in frame_specs:
        entry = {"fn": fn_name, "locals": names(items)}
        if comp:
            entry["comp"] = names(comp)
        frames_out.append(entry)
    return frames_out, heap

# Walk frame.f_back to collect every traced-file frame, innermost first.
def _syn_collect_frames(frame):
    specs = []
    cur = frame
    while cur is not None:
        if cur.f_code.co_filename == "<traced>":
            local = cur.f_locals
            if cur.f_code.co_name == "<module>" and local is not cur.f_globals:
                # Mid-comprehension at module scope (3.12+ inlines it). f_locals is then a proxy
                # over ONLY the comprehension's variables, and reading it as the module's would
                # empty the Global frame of every name the reader defined until the loop ends.
                # Outside a comprehension a module's f_locals IS its globals, so this is exact.
                specs.append((cur.f_code.co_name, list(cur.f_globals.items()), list(local.items())))
            else:
                # Inside a FUNCTION, 3.12+ keeps a comprehension's variable as one of the
                # function's own fast locals, so it already shows beside the others — and a name
                # can be both a comprehension's target and an ordinary local of the same
                # function, which no split by name could tell apart.
                specs.append((cur.f_code.co_name, list(local.items()), []))
        cur = cur.f_back
    return specs

class _SynAwaitInput(BaseException):
    """Stdin ran dry. BaseException, not Exception, so a user's `except Exception` cannot
    swallow the one signal that tells the client to ask for another line."""

def _syn_input(prompt=""):
    # The prompt is the program's OUTPUT, so it is written even on the turn that stops —
    # otherwise the reader is asked for a value with no idea what it is for.
    if prompt != "":
        sys.stdout.write(str(prompt))
    line = sys.stdin.readline()
    if line == "":
        _syn_waiting[0] = True
        _syn_prompt[0] = str(prompt)
        raise _SynAwaitInput()
    value = line[:-1] if line.endswith("\n") else line
    # WHICH step read it, not merely that it was read. A reader scrubbing backwards is standing
    # before some of these values were served, and a log that struck them all through would claim
    # the program knew them all along. The step is the one already recorded for the line calling
    # input(): this wrapper is not traced, so nothing is appended between that step and here.
    _syn_inputs.append({"v": value, "at": max(len(_syn_steps) - 1, 0)})
    return value

def _syn_tracer(frame, event, arg):
    # Once the program has asked for input nobody could serve, everything that follows is
    # _SynAwaitInput travelling back out of the stack — a `return` per frame, at the line that
    # asked. Recording those would end the story on a return the reader never wrote, and would
    # leave the last two steps sitting on the same line. The step that CALLED input() is the
    # last true one, and it is the one to stop on.
    if _syn_waiting[0]:
        return _syn_tracer
    if frame.f_code.co_filename != "<traced>":
        return _syn_tracer
    if event == "call" and not (frame.f_code.co_flags & _SYN_CO_OPTIMIZED) \
            and frame.f_code.co_name != "<module>":
        # A class BODY. It runs once, at the `class` line, and walking it shows the reader a frame
        # named after their class stepping through `def` lines that define rather than run.
        # Returning None leaves this frame untraced; its methods are traced when called.
        return None
    if event == "exception":
        # The FIRST frame of a propagation, not the last. An exception fires this event again in
        # every frame it passes through on its way out, so overwriting would land on the outermost
        # one — past the phantom returns this exists to trim, which is how the innermost frame
        # kept a `Return value: None` it never returned.
        if _syn_raised_at[0] is None:
            _syn_raised_at[0] = max(len(_syn_steps) - 1, 0)
        return _syn_tracer
    if event in ("line", "call"):
        # Execution resumed, so whatever was propagating got caught: only an exception that never
        # stops propagating ends the run. An unwind fires nothing but `exception` and `return`, so
        # reaching either of these means the story carried on.
        _syn_raised_at[0] = None
    if event in ("line", "call", "return"):
        if frame.f_lineno <= 0:
            return _syn_tracer
        try:
            specs = _syn_collect_frames(frame)
            if event == "return" and specs and frame.f_code.co_name != "<module>":
                # What the frame is handing back, as a synthetic local on the frame doing the
                # handing — so it travels through the same snapshot as every other value and an
                # object returned draws its arrow like any other. The space in the name is what
                # keeps it from ever colliding with something the reader wrote. The MODULE is
                # exempt: its implicit None at the end of the file answers a question nobody
                # asked, and it lands on the Global frame where every real name lives.
                name, items, comp = specs[0]
                specs[0] = (name, items + [("Return value", arg)], comp)
            frames_data, heap = _syn_snapshot(specs)
            _syn_steps.append({
                "line": frame.f_lineno,
                "event": event,
                "frames": frames_data,
                "heap": heap,
                "out": _syn_stdout.written,
            })
        except Exception:
            pass
        if len(_syn_steps) >= _syn_step_limit:
            _syn_truncated[0] = True
            sys.settrace(None)
    return _syn_tracer

try:
    _syn_compiled = None
    try:
        _syn_compiled = compile(_syn_source, "<traced>", "exec")
    except SyntaxError as _syn_bad:
        # Nothing runs, so there is nothing to trace — and the reason is then the ONLY useful
        # thing this run can report. Dropped, it leaves the reader an empty canvas and no clue.
        _syn_error[0] = {"type": type(_syn_bad).__name__,
                         "message": str(_syn_bad)[:200],
                         "line": _syn_bad.lineno or 0}
    if _syn_compiled is not None:
        _syn_ns = {"__name__": "__main__", "input": _syn_input}
        sys.settrace(_syn_tracer)
        try:
            exec(_syn_compiled, _syn_ns)
        except _SynAwaitInput:
            # Not a failure: the program got as far as the input it is waiting for, and every
            # step up to it is worth showing.
            pass
        except BaseException as _syn_dead:
            # The program died. Everything the tracer recorded after the raise is that exception
            # travelling back out — a `return` per frame, at the line that raised — so the last
            # TRUE step is the one that raised, and the rest is an artifact of how we watch.
            if _syn_raised_at[0] is not None:
                del _syn_steps[_syn_raised_at[0] + 1:]
            _syn_error[0] = {"type": type(_syn_dead).__name__,
                             "message": str(_syn_dead)[:200],
                             "line": _syn_steps[-1]["line"] if _syn_steps else 0}
        finally:
            sys.settrace(None)
finally:
    while True:
        _syn_payload = json.dumps({"steps": _syn_steps, "truncated": _syn_truncated[0],
                                   "inputs": _syn_inputs, "waiting": _syn_waiting[0],
                                   "prompt": _syn_prompt[0], "error": _syn_error[0]})
        if len(_syn_payload) <= _syn_max_payload or len(_syn_steps) <= 1:
            break
        _syn_steps = _syn_steps[:-(len(_syn_steps) // 4 + 1)]
        _syn_truncated[0] = True
    sys.stdout.write("\n__SYNAPSE_HEAP_BEGIN__")
    sys.stdout.write(_syn_payload)
    sys.stdout.write("__SYNAPSE_HEAP_END__\n")
