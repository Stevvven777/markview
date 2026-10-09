# Flowchart 图表

```mermaid
flowchart LR
  subgraph Reader
    A[Markdown 中文] -->|Parse| B{Layout?}
    B -->|Yes| C[GPU]
    B -->|No| D[Diagnostic]
  end
  C --> E((Read))
```
