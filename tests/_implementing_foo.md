---
title: This is how to implement foo
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
