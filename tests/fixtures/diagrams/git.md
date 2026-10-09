# Git branches

```mermaid
gitGraph
  commit id: "first"
  branch feature
  checkout feature
  commit id: "change" tag: "v1"
  checkout main
  commit id: "main"
  merge feature
```
