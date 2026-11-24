/**
 * What the code lab offers per language: its name on the menu, the program a fresh tab opens on,
 * and the file extension it downloads and opens as.
 *
 * The list is the client's own runnable vocabulary (`lib/execution/language.ts` `LANGUAGES`,
 * mirroring the server's `Language::ALL`), so the menu can never offer a language go-judge has
 * no recipe for. Each starter compiles under that language's recipe as-is: Java's class is `Main`
 * because the recipe compiles `Main.java`, and SQL is SQLite's dialect against `:memory:`.
 */
import { LANGUAGES } from "../../lib/execution/language";

export interface LanguageInfo {
  /** The canonical token the server resolves — `cpp`, not `c++`. */
  readonly id: string;
  readonly label: string;
  readonly extension: string;
}

const INFO: Record<string, { label: string; extension: string }> = {
  python: { label: "Python", extension: "py" },
  java: { label: "Java", extension: "java" },
  scala: { label: "Scala", extension: "scala" },
  c: { label: "C", extension: "c" },
  cpp: { label: "C++", extension: "cpp" },
  go: { label: "Go", extension: "go" },
  rust: { label: "Rust", extension: "rs" },
  kotlin: { label: "Kotlin", extension: "kt" },
  typescript: { label: "TypeScript", extension: "ts" },
  javascript: { label: "JavaScript", extension: "js" },
  sql: { label: "SQL", extension: "sql" },
};

/** Every language the lab runs, in the server's order. */
export const LAB_LANGUAGES: readonly LanguageInfo[] = LANGUAGES.map(([id]) => ({
  id,
  label: INFO[id]?.label ?? id,
  extension: INFO[id]?.extension ?? "txt",
}));

export const DEFAULT_LANGUAGE = "python";

export function languageInfo(id: string): LanguageInfo {
  return LAB_LANGUAGES.find((info) => info.id === id) ?? LAB_LANGUAGES[0]!;
}

/** The language a file opens as, from its extension; null for one the lab cannot run. */
export function languageForFile(name: string): string | null {
  const dot = name.lastIndexOf(".");
  if (dot < 0) return null;
  const ext = name.slice(dot + 1).toLowerCase();
  if (ext === "h") return "c";
  if (ext === "cc" || ext === "cxx" || ext === "hpp") return "cpp";
  if (ext === "mjs" || ext === "cjs") return "javascript";
  return LAB_LANGUAGES.find((info) => info.extension === ext)?.id ?? null;
}

const STARTERS: Record<string, string> = {
  python: `def greet(name: str) -> str:
    return f"Hello, {name}!"


print(greet("World"))
`,
  java: `public class Main {
    public static void main(String[] args) {
        System.out.println("Hello, World!");
    }
}
`,
  scala: `@main def hello(): Unit =
  println("Hello, World!")
`,
  c: `#include <stdio.h>

int main(void) {
    printf("Hello, World!\\n");
    return 0;
}
`,
  cpp: `#include <iostream>

int main() {
    std::cout << "Hello, World!" << std::endl;
    return 0;
}
`,
  go: `package main

import "fmt"

func main() {
	fmt.Println("Hello, World!")
}
`,
  rust: `fn main() {
    println!("Hello, World!");
}
`,
  kotlin: `fun main() {
    println("Hello, World!")
}
`,
  typescript: `const greet = (name: string): string => \`Hello, \${name}!\`;

console.log(greet("World"));
`,
  javascript: `const greet = (name) => \`Hello, \${name}!\`;

console.log(greet("World"));
`,
  sql: `CREATE TABLE greeting (message TEXT);
INSERT INTO greeting VALUES ('Hello, World!');
SELECT message FROM greeting;
`,
};

/** The program a fresh tab in `language` opens on. */
export function starterFor(language: string): string {
  return STARTERS[language] ?? "";
}
