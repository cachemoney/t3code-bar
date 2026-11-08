import json
import math
import sys


def compare(actual, expected, path="$"):
    if isinstance(actual, dict) and isinstance(expected, dict):
        if actual.keys() != expected.keys():
            raise AssertionError(f"{path}: keys differ: {actual.keys() ^ expected.keys()}")
        for key in actual:
            compare(actual[key], expected[key], f"{path}.{key}")
    elif isinstance(actual, list) and isinstance(expected, list):
        if len(actual) != len(expected):
            raise AssertionError(f"{path}: lengths differ")
        for index, (a, b) in enumerate(zip(actual, expected)):
            compare(a, b, f"{path}[{index}]")
    elif type(actual) in (int, float) and type(expected) in (int, float):
        if not math.isclose(actual, expected, rel_tol=0, abs_tol=1e-9):
            raise AssertionError(f"{path}: {actual} != {expected}")
    elif actual != expected:
        raise AssertionError(f"{path}: {actual} != {expected}")


with open(sys.argv[1]) as actual_file, open(sys.argv[2]) as expected_file:
    compare(json.load(actual_file), json.load(expected_file))
print("Export matches within 1e-9 absolute tolerance.")
