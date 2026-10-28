# go-judge sandbox + Synapse language toolchains.
#
# Base: criyle/go-judge (Debian 13 "trixie", binary /opt/go-judge, listens :5050).
# go-judge bundles NO compilers — the sandbox bind-mounts the container's
# /usr (incl. /usr/local), /bin, /lib, /lib64, /etc/alternatives READ-ONLY (see
# /opt/mount.yaml) and runs user code as uid 1536. CONSEQUENCES for this image:
#   * every toolchain MUST live under a mounted path — /usr or /usr/local — NEVER
#     /opt or /root (those are not mounted into the sandbox).
#   * binaries/libs must be world-readable/executable (sandbox user is uid 1536).
#   * runtime has NO network (go-judge isolates per submission) and writable dirs
#     are only /w (workDir) and /tmp (tmpfs) — both via HOME=/w at run time.
#
# The toolchains mirror the languages in synapse.server.execution.domain.Language:
# python, java, scala, c, c++, go, rust, kotlin, typescript, javascript, sql.
# JDK 25, the only JDK: Java, Kotlin and Scala all run on it. A second JDK would
# not stay idle — `update-alternatives` points /usr/bin/java at the newest one
# installed. The JDK reads its config from /etc/java-25-openjdk, which the stock
# mount table does not bind, so this image ships its own (mount.yaml).
FROM criyle/go-judge:latest

COPY mount.yaml /opt/mount.yaml

# Declared only so the shared release engine can pass --build-arg SYNAPSE_VERSION to every
# image it builds without docker warning about an unconsumed arg. The sandbox does not use it.
ARG SYNAPSE_VERSION=unknown

ENV DEBIAN_FRONTEND=noninteractive

# apt toolchains — all land in /usr (mounted). rustc via apt (NOT rustup, which
# installs to /root/.cargo — invisible to the sandbox). nodejs+npm: the base has
# no Node, needed for JavaScript/TypeScript.
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl wget unzip \
      python3 \
      build-essential \
      golang \
      openjdk-25-jdk-headless \
      sqlite3 \
      rustc \
      nodejs npm \
 && rm -rf /var/lib/apt/lists/*

# TypeScript + tsx. Debian's npm global prefix is /usr/local → installs under
# /usr (mounted). tsx runs .ts directly.
RUN npm install -g typescript tsx && npm cache clean --force

# Kotlin compiler → /usr/local (NOT /opt). kotlinc is a JVM launcher; needs java
# (present) at run time, no network for a single-file compile.
ENV KOTLIN_VERSION=2.2.21
RUN curl -fsSL "https://github.com/JetBrains/kotlin/releases/download/v${KOTLIN_VERSION}/kotlin-compiler-${KOTLIN_VERSION}.zip" -o /tmp/kotlin.zip \
 && unzip -q /tmp/kotlin.zip -d /usr/local \
 && ln -sf /usr/local/kotlinc/bin/kotlinc /usr/local/bin/kotlinc \
 && ln -sf /usr/local/kotlinc/bin/kotlin  /usr/local/bin/kotlin \
 && chmod -R a+rX /usr/local/kotlinc \
 && rm /tmp/kotlin.zip

# scala-cli (glibc static binary) → /usr/local/bin.
# ⚠ Scala caveat: scala-cli fetches the Scala toolchain via coursier at run
# time (network). go-judge sandboxes have NO network, so the cache is pre-warmed
# below into a mounted, uid-1536-readable path so run-time compiles are offline.
# 1.18.0 defaults to Scala 3.9: older Scala's LazyVals call sun.misc.Unsafe, which
# JDK 25 reports on stderr in every run — from the compiler's own JVM, which no
# --java-opt reaches.
ENV SCALA_CLI_VERSION=1.18.0
RUN ARCH="$(uname -m)" \
 && case "$ARCH" in \
      x86_64)  SCALA_ARCH=x86_64-pc-linux ;; \
      aarch64) SCALA_ARCH=aarch64-pc-linux ;; \
      *) echo "unsupported arch: $ARCH"; exit 1 ;; \
    esac \
 && curl -fsSL "https://github.com/VirtusLab/scala-cli/releases/download/v${SCALA_CLI_VERSION}/scala-cli-${SCALA_ARCH}.gz" -o /tmp/scala-cli.gz \
 && gunzip /tmp/scala-cli.gz \
 && mv /tmp/scala-cli /usr/local/bin/scala-cli \
 && chmod 755 /usr/local/bin/scala-cli

# Pre-warm the Scala 3 toolchain so `scala run` works OFFLINE (the sandbox has NO
# network — that UnknownHostException was scala-cli fetching coursier's jvm-index
# at run time). Two parts, both consumed by the Scala run command in GoJudgeSpec:
#   * --jvm system → use the installed openjdk-25 (no JVM-index fetch), and
#   * COURSIER_CACHE under /usr/local (a mounted, uid-1536-readable path) populated
#     HERE at build time (network available) with the Scala 3 compiler + stdlib,
#     so run-time compiles are pure cache hits.
ENV COURSIER_CACHE=/usr/local/share/coursier
RUN mkdir -p "$COURSIER_CACHE" \
 && printf '@main def __warm(): Unit = println("scala toolchain warm")\n' > /tmp/warm.scala \
 && HOME=/tmp scala-cli run /tmp/warm.scala --quiet --server=false --jvm system \
 && chmod -R a+rX "$COURSIER_CACHE" \
 && rm -rf /tmp/warm.scala /tmp/.scala-build /tmp/.cache

# Inherit ENTRYPOINT ["/opt/go-judge"], WORKDIR /opt, port 5050 from the base.
