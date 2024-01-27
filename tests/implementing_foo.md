---
title: This is how to implement foo
author: Crax
publish_date: 2024-1-12T09:00:00Z
---

# Way 1

```c
int foo() {
    return 42;
}
```

# Way 2

```c
int foo() {
    return baz() ? 42 : 41;
}
```
