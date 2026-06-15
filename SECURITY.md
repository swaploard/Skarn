# Security Policy

Skarn exists to make running AI-agent commands and **LLM-generated code**
safer. It also *is* a program that deliberately executes untrusted code, so its
threat model must be explicit. Please read this before relying on it.

## What Skarn defends against

- **Destructive filesystem writes.** A sandboxed command or Code Mode script can
  only write inside the workspace you grant it. `rm -rf ~`, overwriting dotfiles,
  or scribbling on system directories are blocked at the kernel level.
- **Unauthorized network egress.** Network access is denied by default, which is
  the primary control against data exfiltration — even if a script reads
  something sensitive, it cannot send it anywhere.
- **Secret disclosure of well-known credential stores.** On macOS, where reads
  are otherwise broad (see below), `~/.ssh`, `~/.aws`, `~/.gnupg`, cloud CLI
  configs, and similar locations are explicitly denied. On Linux the allow-list
  model means only granted paths are readable at all.
- **Code Mode escapes — defense in depth, not the boundary.** Before any script
  runs it is parsed with `oxc` and rejected if it references `eval`, `Function`,
  `require`, `import`, `process`, `Deno`, `globalThis`, `Reflect`, or the
  `.constructor`/`.__proto__`/`.prototype` properties (whether by dot or bracket
  access, e.g. `x["constructor"]`). Because we ban the *identifiers*, alias hops
  (`const e = eval; e(...)`) are caught too. This static pass exists to reject
  obviously hostile scripts early; the *actual* guarantees are the hermetic
  context and the OS sandbox below — neither relies on the validator being
  complete.
- **Resource exhaustion.** Code Mode execution is bounded by a memory limit, a
  native stack limit, a wall-clock deadline (with an uncatchable interrupt), a
  tool-call budget, and an output-size cap.
- **Context poisoning.** Intermediate tool results stay inside the isolate; only
  the explicitly returned value and `skarn.log` lines leave it.

## Defense in depth

Two independent layers protect Code Mode execution:
