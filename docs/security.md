# Security and threat model

Markview aims to make opening an untrusted Markdown document safe. This page defines the security policy, attacker model, trust boundaries, and accepted risks. It describes design goals and controls, not a proof that every input is harmless.

[Security reference](security-reference.md) owns the threat catalog, implementation details, limits, and historical findings. [Security verification](security-verification.md) tracks evidence and outstanding verification work. [Architecture](architecture.md) explains pipeline ownership and resource boundaries.

## Reporting a Vulnerability

If you believe you have found a security vulnerability in Markview, please report it *privately*.
The preferred method is to open a GitHub Security Advisory (Repository → Security → Advisories).

## Scope and assumptions

The model covers a user on an unsandboxed desktop opening untrusted Markdown: downloads, mail attachments, extracted archives, generated text, shared folders, and documents reached through links. Parsing, layout, image loading, and watched revisions are in scope for both the reader and PDF export.

Documents do not execute scripts. The supported raw HTML subset expresses Markdown-like semantics; `class` and `style` attributes are not interpreted. Document-controlled paths and URLs reach the filesystem, network, and operating-system handlers only through the policies below. Installed stylesheets are user-selected resources with a separate trust boundary; their declared fonts are downloaded only on explicit request.

PDF export writes to the destination the user supplies, and `pdf --watch` rewrites that same destination. Documents cannot choose an additional output path. Export runs without a window, GPU, or link handler and shares the reader's parsing and image budgets. A downstream PDF viewer's handling of the output belongs to that viewer's boundary.

Physical access, kernel or driver defects in themselves, dependency supply-chain compromise, and social engineering that persuades a user to install or approve something are outside this model. Vulnerabilities in dependencies reached by document input remain relevant to the reader's safety.

## Assets

| | Asset | Consequence of compromise |
| --- | --- | --- |
| A1 | Process availability | Loss of what the reader is reading, unsaved session state, an interrupted `--watch` session |
| A2 | Host confidentiality | Local file contents read or rendered |
| A3 | Host integrity | Arbitrary code execution |
| A4 | Network identity | Opening a document alone discloses IP address and online activity |
| A5 | Trust in the reader's own interface | Document content impersonating reader chrome or a security prompt |
| A6 | Clipboard and selected text | Injection through the paste path |

## Attacker capabilities

| | Capability | Realistic setting |
| --- | --- | --- |
| K0 | Controls the `.md` bytes | The baseline; always assume it |
| K1 | Also controls other files reachable by relative path | Extracted archive, cloned repository, shared folder |
| K2 | Controls a remote server the document points at | Common |
| K3 | Controls a stylesheet the user loads | Depends on distribution |

## Trust boundaries

```text
   .md bytes ──(B1)──► comrak ──► Document (pure data) ──(B2)──► Layout ──► Scene ──► GPU (B4)
      │                    │                                            ▲
      │                    ├─► fence info ──► syntect ────────────────┘
      │                    ├─► $...$ ───────► ratex ──────────────────┘
      │                    └─► img src ──┐
      │                                  ▼
      └────────────────────────► (B3) image source resolution ──► local file / HTTP / data:

   link click ────────────────────► (B5) src/link.rs ──► OS handler

B1 untrusted bytes → pure data        B2 pure data → geometry (no side effects)
B3 document → filesystem and network  B4 process → driver
B5 document → OS execution
```

Image resolution and link activation cross the filesystem, network, and OS-handler boundaries. Parsing, shaping, layout, and decoding also expose availability and dependency-safety risks even when they perform no external I/O. The GPU is a separate process-to-driver boundary.

## Security policy

### Document processing and resource budgets

Untrusted input should not crash, hang, or exhaust the reader. Input-dependent recursion and expensive rendering work use explicit budgets; the reader degrades presentation or reports a resource error when a limit is reached. Decoded images, concurrent image work, and cache residency are bounded. These controls do not establish a universal bound on total process memory or execution time.

Document rendering does not grant content authority to alter reader controls. Link confirmations identify the resolved target rather than trusting the document's label. Copied text remains document-controlled data; copying does not make it safe to execute elsewhere.

### Image paths

Local image sources must be relative paths. Absolute paths and `file:` image URLs are refused. Relative paths may contain `../` and may follow symlinks outside the document directory: directory containment is deliberately not a security boundary, because neighbouring image directories are an ordinary use case.

Local image reads do not feed file contents into remote URLs. Remote HTTP(S) images and bounded `data:image/` sources are separate supported source types; SVG images cannot load additional resources through their image-href resolver.

### Links and OS handlers

Clicked Markdown links open in Markview. Directories and a closed allowlist of file extensions open through the OS without confirmation; expanding that list is a security-policy change. Other local file types require a blocking confirmation with a safe default, no remembered exemption, and the resolved path rather than the link label. The user may open the containing folder, explicitly open the file, or close the prompt.

HTTP, HTTPS, and mailto links may open through their OS handlers on a click. Other remote schemes are refused. Local links, including `file:` links, are classified under the local-file policy and are not confined to the document directory. Existing targets are canonicalized before classification. The [reference](security-reference.md#local-link-classification) records the precise extension list and classification limitations.

### Network access and caching

Remote images load by default, subject to a per-document-revision source cap. Excess sources remain placeholders; a notice offers Dismiss or Load all. Load all lifts only that tab's source cap for that content revision. The notice is not a consent gate for requests within the cap.

Connections refuse private, loopback, link-local, and other non-public address classes after DNS resolution. Addresses are pinned for connection, and every redirect repeats the check. This policy also applies after Load all; local-network image services are deliberately unsupported.

Image requests have byte and time limits. Cached image bodies and their URLs persist in a bounded disk cache. `--offline` makes no image requests and may serve cached bodies even when stale, including entries whose cache directives would otherwise require revalidation.

### Font downloads

Stylesheet-declared fonts download only through an explicit Fonts-page action or `markview fonts download`, never just by opening or installing a stylesheet. Downloads share the network address policy and are refused under `--offline`.

Downloads and archive extraction are bounded. Extraction cannot escape the download directory or install symlinks or hard links. Declared hashes must match, and font contents are validated before installation. HTTPS is preferred; HTTP mirrors are accepted with the corresponding transport privacy and integrity risk. Installed fonts are personal resources; measurement modes and explicitly isolated font configurations retain their configured font set.

## Accepted and residual risks

- **Network identity and history:** default remote-image requests can disclose an IP address, opening time, and a document-specific URL without prior confirmation. The source cap limits automatic activity; offline mode prevents it. Cached bodies and URLs leave a local record readable by anyone with access to the cache.
- **Local reads and races:** relative image paths, symlinks, and clicked local links can reach outside the document directory. Sensitive images may be displayed, and errors can reveal local existence or file type. A local writer can race reads or preserve file metadata used for staleness checks. Directory containment is intentionally absent; no mechanism sends the resulting file contents back in a URL.
- **OS handlers and user decisions:** confirmation permits potentially executable files to open. Allowlisted viewers also have their own vulnerabilities; an SVG opened as a top-level browser document may execute script. Extension classification is not a proof that a file or its handler is safe.
- **Resource and dependency safety:** byte and work budgets reduce pathological workloads but cannot guarantee fast math evaluation or bounded aggregate memory. Dependency bugs, malformed geometry, and concurrency failures remain possible; their evidence and verification gaps belong in the verification document.
- **Interface impersonation:** headings, link labels, and image alt text remain attacker-controlled. Restricted HTML and target-based confirmations reduce the surface but do not authenticate displayed content.

## Non-goals

Markview does not make a hostile document safe to act on, confine local paths to a document subtree, or protect against an adversary who already controls the user's files beyond the controls described above. It does not prohibit misleading content from being displayed or guarantee the safety of external handlers, downstream PDF viewers, or drivers.
