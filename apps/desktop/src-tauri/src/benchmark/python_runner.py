"""Trusted observation shim, executed ONLY inside a restricted Linux container.

Expected answers and final pass/fail decisions remain in the Rust controller.
"""
import ast
import contextlib
import copy
import io
import json
import sys


class OutputLimit(RuntimeError):
    pass


class Capture(io.StringIO):
    def __init__(self):
        super().__init__()
        self.byte_count = 0

    def write(self, value):
        self.byte_count += len(value.encode("utf-8"))
        if self.byte_count > 65536:
            raise OutputLimit("output_limit")
        return super().write(value)


def typed(value):
    if type(value) in (type(None), bool, int, str):
        return {"type": type(value).__name__, "value": value}
    if type(value) is list:
        return {"type": "list", "items": [typed(v) for v in value]}
    return {"type": "unsupported", "python_type": type(value).__name__}


def main():
    payload = json.loads(sys.stdin.buffer.read().decode("utf-8"))
    # Save controller operations before evaluating the submitted function.
    emit, output = json.dumps, sys.stdout
    output.write(emit({"protocol": 1, "event": "python_ready", "python": list(sys.version_info[:2])}) + "\n")
    output.flush()
    if sys.version_info[:2] != (3, 13):
        return
    captured = Capture()
    try:
        tree = ast.parse(payload["code"], filename="submission.py")
        for node in tree.body:
            if not isinstance(node, (ast.FunctionDef, ast.Import, ast.ImportFrom)):
                raise ValueError("only_function_definitions_and_imports")
        if any(isinstance(n, (ast.Global, ast.Nonlocal)) for n in ast.walk(tree)):
            raise ValueError("global_state_not_allowed")
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                if any(n.name.split(".")[0] not in sys.stdlib_module_names for n in node.names):
                    raise ValueError("standard_library_imports_only")
            elif isinstance(node, ast.ImportFrom):
                if node.level or not node.module or node.module.split(".")[0] not in sys.stdlib_module_names:
                    raise ValueError("standard_library_imports_only")
        args = payload["args"]
        before = copy.deepcopy(args)
        space = {}
        with contextlib.redirect_stdout(captured), contextlib.redirect_stderr(captured):
            exec(compile(tree, "submission.py", "exec"), space)
            fn = space.get(payload["function"])
            if not callable(fn):
                raise ValueError("function_missing")
            result = fn(*args)
        observed = {"protocol": 1, "python": list(sys.version_info[:2]), "outcome": "returned", "return": typed(result), "arguments": typed(args), "input_unchanged": args == before, "extra_output": bool(captured.getvalue())}
    except BaseException as error:
        observed = {"protocol": 1, "python": list(sys.version_info[:2]), "outcome": "program_error", "error_type": type(error).__name__}
    output.write(emit(observed, ensure_ascii=True, allow_nan=False))
    output.flush()


if __name__ == "__main__":
    main()
