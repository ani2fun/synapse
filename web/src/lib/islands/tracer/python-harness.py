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
import sys, json, base64, math, types

_syn_source = base64.b64decode("__SYNAPSE_USER_SOURCE_B64__").decode("utf-8")
_syn_steps = []
_syn_truncated = [False]
_syn_step_limit = 600
_syn_max_objects = 400
_syn_max_depth = 60
_syn_max_payload = 512 * 1024

# Modules whose objects are stdlib/library internals, not user data — render their
# instances opaque (no field recursion) so importing `deque`/`Optional` doesn't drag
# the metaclass tree into every snapshot.
_syn_opaque_modules = frozenset((
    "typing", "_collections_abc", "collections.abc", "abc",
    "_typeshed", "_collections", "_weakrefset", "weakref",
))

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

def _syn_scalar(v):
    if v is None or isinstance(v, bool) or isinstance(v, int):
        return (True, v)
    if isinstance(v, float):
        return (True, v if math.isfinite(v) else repr(v))
    if isinstance(v, str):
        return (True, v if len(v) <= 80 else v[:80] + "…")
    return (False, None)

# Snapshot the call stack (a list of (fn_name, locals_items), innermost first) into the
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
        if _syn_is_opaque(v):
            heap[oid] = {"type": "object", "cls": type(v).__name__, "fields": {}}
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
    frames_out = []
    for fn_name, items in frame_specs:
        locs = {}
        for k, v in items:
            if isinstance(k, str) and not k.startswith("_syn_") and not k.startswith("__"):
                locs[k] = visit(v, 0)
        frames_out.append({"fn": fn_name, "locals": locs})
    return frames_out, heap

# Walk frame.f_back to collect every traced-file frame, innermost first.
def _syn_collect_frames(frame):
    specs = []
    cur = frame
    while cur is not None:
        if cur.f_code.co_filename == "<traced>":
            specs.append((cur.f_code.co_name, list(cur.f_locals.items())))
        cur = cur.f_back
    return specs

def _syn_tracer(frame, event, arg):
    if event in ("line", "call", "return") and frame.f_code.co_filename == "<traced>":
        if frame.f_lineno <= 0:
            return _syn_tracer
        try:
            frames_data, heap = _syn_snapshot(_syn_collect_frames(frame))
            _syn_steps.append({
                "line": frame.f_lineno,
                "event": event,
                "frames": frames_data,
                "heap": heap,
            })
        except Exception:
            pass
        if len(_syn_steps) >= _syn_step_limit:
            _syn_truncated[0] = True
            sys.settrace(None)
    return _syn_tracer

try:
    _syn_compiled = compile(_syn_source, "<traced>", "exec")
    _syn_ns = {"__name__": "__main__"}
    sys.settrace(_syn_tracer)
    try:
        exec(_syn_compiled, _syn_ns)
    finally:
        sys.settrace(None)
finally:
    while True:
        _syn_payload = json.dumps({"steps": _syn_steps, "truncated": _syn_truncated[0]})
        if len(_syn_payload) <= _syn_max_payload or len(_syn_steps) <= 1:
            break
        _syn_steps = _syn_steps[:-(len(_syn_steps) // 4 + 1)]
        _syn_truncated[0] = True
    sys.stdout.write("\n__SYNAPSE_HEAP_BEGIN__")
    sys.stdout.write(_syn_payload)
    sys.stdout.write("__SYNAPSE_HEAP_END__\n")
