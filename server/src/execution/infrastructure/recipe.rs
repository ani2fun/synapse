//! Per-language sandbox recipes (oracle: `GoJudgeRecipe.scala`). go-judge is a raw command
//! runner with NO language abstraction — compile/run orchestration lives in the adapter. The
//! `match` is exhaustive on purpose: adding a `Language` won't compile until it gets a recipe.

use crate::execution::domain::Language;

pub(crate) struct Recipe {
    pub(crate) source_file: &'static str,
    pub(crate) compile: Option<&'static str>,
    pub(crate) run: &'static str,
    pub(crate) cpu_seconds: u64,
    pub(crate) clock_seconds: u64,
    pub(crate) memory_mib: u64,
}

const DEFAULT_CPU: u64 = 15;
const DEFAULT_CLOCK: u64 = 30;
const DEFAULT_MEMORY_MIB: u64 = 512;

impl Recipe {
    const fn interpreted(source_file: &'static str, run: &'static str) -> Self {
        Self {
            source_file,
            compile: None,
            run,
            cpu_seconds: DEFAULT_CPU,
            clock_seconds: DEFAULT_CLOCK,
            memory_mib: DEFAULT_MEMORY_MIB,
        }
    }

    const fn compiled(source_file: &'static str, compile: &'static str, run: &'static str) -> Self {
        Self {
            source_file,
            compile: Some(compile),
            run,
            cpu_seconds: DEFAULT_CPU,
            clock_seconds: DEFAULT_CLOCK,
            memory_mib: DEFAULT_MEMORY_MIB,
        }
    }

    pub(crate) fn for_language(language: Language) -> Recipe {
        match language {
            Language::Python => Self::interpreted("main.py", "python3 main.py"),
            Language::Java => Self::compiled("Main.java", "javac Main.java", "java -cp . Main"),
            Language::Scala => Recipe {
                cpu_seconds: 60,
                clock_seconds: 120,
                memory_mib: 1024,
                ..Self::interpreted(
                    "main.scala",
                    "COURSIER_CACHE=/usr/local/share/coursier scala-cli run main.scala --quiet \
                     --server=false --jvm system --java-opt -Dstdout.encoding=UTF-8 --java-opt \
                     -Dstderr.encoding=UTF-8",
                )
            },
            Language::C => Self::compiled("main.c", "gcc -O2 main.c -o __cf_bin", "./__cf_bin"),
            Language::Cpp => Self::compiled("main.cpp", "g++ -O2 main.cpp -o __cf_bin", "./__cf_bin"),
            Language::Go => Self::compiled("main.go", "go build -o __cf_bin main.go", "./__cf_bin"),
            Language::Rust => Self::compiled("main.rs", "rustc -O main.rs -o __cf_bin", "./__cf_bin"),
            Language::Kotlin => Recipe {
                cpu_seconds: 60,
                clock_seconds: 90,
                memory_mib: 1024,
                ..Self::compiled(
                    "main.kt",
                    "kotlinc main.kt -include-runtime -d __cf.jar",
                    "java -jar __cf.jar",
                )
            },
            Language::TypeScript => Self::interpreted("main.ts", "tsx main.ts"),
            Language::JavaScript => Self::interpreted("main.js", "node main.js"),
            Language::Sql => Self::interpreted("main.sql", "sqlite3 :memory: < main.sql"),
        }
    }
}
