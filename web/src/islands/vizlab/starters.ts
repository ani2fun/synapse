// The programs the lab opens on — one per traceable language, both drawing the SAME picture: a
// two-pointer reverse over `arr`, which is the canonical `array` trace (left/right carets march
// inward, each swap rings the cells it touched). Switching language should change the syntax and
// nothing else, so a reader can tell the two apart at a glance.
//
// They are seeds, not fixtures: the buffer autosaves from the first keystroke, and a reader who
// has edited never sees these again.

/** The structure the starters are written for — the picker opens here. */
export const STARTER_HINT = "array:arr";

export const STARTERS: Record<string, string> = {
  python: `arr = [5, 2, 8, 1, 9, 3]
left, right = 0, len(arr) - 1
while left < right:
    arr[left], arr[right] = arr[right], arr[left]
    left += 1
    right -= 1
print(arr)
`,
  java: `public class Main {
    public static void main(String[] args) {
        int[] arr = {5, 2, 8, 1, 9, 3};
        int left = 0, right = arr.length - 1;
        while (left < right) {
            int t = arr[left];
            arr[left] = arr[right];
            arr[right] = t;
            left++;
            right--;
        }
        System.out.println(java.util.Arrays.toString(arr));
    }
}
`,
};

/** The order the language tabs appear in, and the default. */
export const LANGUAGES = ["python", "java"] as const;
