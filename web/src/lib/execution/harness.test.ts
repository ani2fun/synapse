import { describe, expect, it } from "vitest";

import { joinHarness, splitHarness, visiblePart } from "./harness";

const JAVA = `import java.io.*;
import java.util.*;

public class Main {
    static class Solution {
        void nextPermutation(int[] nums) {
            String s = "}{";   // braces in a literal never close the class
            char c = '}';
        }
    }

    public static void main(String[] args) throws IOException {
        System.out.println(show(new int[0]));
    }
}`;

const CPP = `#include <vector>

using namespace std;

class Solution {
public:
    int answer(vector<int> &nums) {
        return 0; // }
    }
};

int main() {
    return 0;
}`;

const PYTHON = `class Solution:
    def answer(self, nums: List[int]) -> int:
        return 0

    # a comment inside the class


def show(v):
    return str(v)


nums = parse(input())
print(show(Solution().answer(nums)))`;

describe("splitHarness", () => {
  it("shows a nested Java Solution dedented and without static", () => {
    const split = splitHarness("java", JAVA)!;
    expect(split.body.split("\n")[0]).toBe("class Solution {");
    expect(split.body.split("\n").at(-1)).toBe("}");
    expect(split.body).toContain('String s = "}{";');
    expect(split.harness.indent).toBe("    ");
    expect(split.harness.staticKw).toBe(true);
    expect(split.harness.suffix).toContain("public static void main");
  });

  it("keeps the C++ class through its closing `};`", () => {
    const split = splitHarness("cpp", CPP)!;
    expect(split.body.startsWith("class Solution {")).toBe(true);
    expect(split.body.endsWith("};")).toBe(true);
    expect(split.harness.prefix).toContain("#include <vector>");
    expect(split.harness.suffix).toContain("int main()");
  });

  it("ends a Python class at the first unindented line", () => {
    const split = splitHarness("python", PYTHON)!;
    expect(split.harness.prefix).toBeNull();
    expect(split.body.endsWith("    # a comment inside the class")).toBe(true);
    expect(split.harness.suffix).toContain("def show(v):");
  });

  it("leaves a program with no single Solution class whole", () => {
    expect(splitHarness("python", "print(1)")).toBeNull();
    expect(splitHarness("python", "class Solution:\n    pass\n\nclass Solution:\n    pass")).toBeNull();
    expect(splitHarness("java", "class MinStack {\n}")).toBeNull();
    expect(splitHarness("go", "class Solution {}\nfunc main() {}")).toBeNull();
  });

  it("finds nothing to hide in a bare class", () => {
    expect(splitHarness("cpp", "class Solution {\n};")).toBeNull();
  });
});

describe("helpers above the class", () => {
  const PY_HELPER = `from typing import List

class TreeNode:
    def __init__(self, val):
        self.val = val

# One queued cell.
class Cell:
    def __init__(self, row):
        self.row = row

class Solution:
    def solve(self) -> int:
        return Cell(0).row

print(Solution().solve())`;

  const JAVA_HELPER = `import java.util.*;

public class Main {
    static class ListNode { int val; }

    static class Cell {
        int row;
    }

    static class Solution {
        int solve() { return new Cell().row; }
    }

    public static void main(String[] args) {
        System.out.println(new Solution().solve());
    }
}`;

  it("shows a Python helper class with the class and keeps platform nodes hidden", () => {
    const split = splitHarness("python", PY_HELPER)!;
    expect(split.body.startsWith("# One queued cell.\nclass Cell:")).toBe(true);
    expect(split.body).toContain("class Solution:");
    expect(split.harness.prefix).toContain("class TreeNode:");
    expect(split.harness.prefix).not.toContain("class Cell");
  });

  it("shows Java helpers without static and restores static on every class", () => {
    const split = splitHarness("java", JAVA_HELPER)!;
    expect(split.body.split("\n")[0]).toBe("class Cell {");
    expect(split.body).toContain("\nclass Solution {");
    expect(split.harness.prefix).toContain("static class ListNode");
    const program = joinHarness("java", split.harness, split.body);
    expect(program).toContain("    static class Cell {");
    expect(program).toContain("    static class Solution {");
    expect(program).toContain("public static void main");
  });

  it("joins a helper the reader writes above their class", () => {
    const split = splitHarness("java", JAVA)!;
    const program = joinHarness("java", split.harness, "class Pair { int a; }\n\nclass Solution {\n}");
    expect(program).toContain("    static class Pair { int a; }");
    expect(program).toContain("    static class Solution {");
  });
});

describe("joinHarness", () => {
  it.each([
    ["java", JAVA],
    ["cpp", CPP],
    ["python", PYTHON],
  ])("rebuilds the authored %s program from the shown class", (language, source) => {
    const split = splitHarness(language, source)!;
    expect(joinHarness(language, split.harness, split.body)).toBe(source);
  });

  it("puts an edited class back into the driver", () => {
    const split = splitHarness("java", JAVA)!;
    const edited = "class Solution {\n    void nextPermutation(int[] nums) {}\n}";
    const program = joinHarness("java", split.harness, edited);
    expect(program).toContain("    static class Solution {\n        void nextPermutation(int[] nums) {}\n    }");
    expect(program).toContain("public static void main");
  });

  it("joins a class the reader gave its own import", () => {
    const split = splitHarness("python", PYTHON)!;
    const program = joinHarness("python", split.harness, "import heapq\n\nclass Solution:\n    pass");
    expect(program.startsWith("import heapq")).toBe(true);
    expect(program).toContain("print(show(Solution().answer(nums)))");
  });

  it("sends a whole program unchanged", () => {
    const split = splitHarness("cpp", CPP)!;
    expect(joinHarness("cpp", split.harness, CPP)).toBe(CPP);
  });

  it("passes code through when there is no harness", () => {
    expect(joinHarness("python", null, "print(1)")).toBe("print(1)");
  });
});

describe("visiblePart", () => {
  it("shows a past whole-program submission as its class", () => {
    expect(visiblePart("cpp", CPP).startsWith("class Solution {")).toBe(true);
  });

  it("leaves a bare class as it is", () => {
    const body = "class Solution:\n    pass";
    expect(visiblePart("python", body)).toBe(body);
  });
});
