# Reference

Everything the language has, in one place. For the rules behind each, see
[the reference](../spec.md).

## Types

| | |
| --- | --- |
| `int` | 64-bit, signed. **Traps** on overflow and on division by zero |
| `string` | compared and passed; never built. No interpolation, no `+` |
| `bool` | `true`, `false` |
| `date`, `datetime` | compared as integers; a duration added to one is one |
| `bytes` | |
| `List<T>` | no index. Consumed by combinators |
| `T?` | narrowed by `exists` in `require` |
| `Credential<T>` | held, not verified. Its claims are out of reach |
| `Verified<P>` | what a `verify` block produces. Names the **policy** |
| `Proof<bool>` | what `prove` produces |

No floating point, anywhere. Money is minor units; a percentage is basis points.

## Declarations

```val
app "reverse.dns.name"
version 1
capabilities { … }
enum Name { a, b }
credential Name { field: type }
type Name { field: type }        // a plain record; nobody signed one
state { field: type default … }
trust Name(subject: Type) [refines Other] { anchor: "…" require { … } }
function name(a: int, b: int): int { … }
action Name { … }
screen Name { … }
```

## Expressions

```val
const x = …                      // no var, no assignment
a ? b : c                        // if is a statement; this is the expression
if (cond) { … } else { … }
switch (x) { A => 1, B => 2, }   // no default over an enum; unreachable arms error
{ ...record, field: value }      // derive; never mutate
x with Policy                    // the only way to get Verified<P>
x exists                         // narrowing, in require
value from { Policy }            // provenance, on an issued claim
```

Arguments are named once there are two: `f(a: 1, b: 2)`. A trailing block is a
block, not an argument.

## Builtins

A closed set. An application cannot add to it — a builtin is the one place a
non-terminating operation could enter a language that has proved it cannot have
one.

```val
duration(days: 30)  duration(hours: 24)  duration(years: 20)
min  max  abs
```

## List combinators

```val
map  filter  fold  any  all  count  first
```

Bounded by the list they visit. No recursion, no loops, no index — every program
halts and the compiler knows it.

## Phases

```
input → require → verify → compute → update → execute
```

Omit any; reorder none. `refuse "key"` is legal before `execute` and not in it.

## Effects

Only in `execute`, never behind a function, offered as one batch.

```val
credential.issue(Type { … })
payment.request(to: …, amount: …)
storage.write(scope: …, id: …, value: …)
message.send(to: …)
network.request(…)
present { disclose … / prove … }
navigate Screen
```

## Screen components

What the host ships, not what the language defines. On Vaulet today:

```val
column { … }
section(text: "key")
card(text: phrase("key", name: value))
tile(text: phrase("key", name: value), onTap: Action)
list(binding) { item -> … }
button(text: "key", emphasis: primary, onTap: Action)
```

Props are semantic. Asking for a component the catalogue does not have is not
drawn approximately — it is reported.

## Screen data

```val
data {
  name: credentials of Type verified with Policy
    order by field desc
    limit 50

  other: query audience.operation(…) as List<Type>
}
```

## The runtime context

```val
context.time.now      context.random.uuid
```

The only sources of nondeterminism, and both are recorded. `Date.now()` and
`random()` do not exist.

## Tools

```bash
valc    file.val …             # diagnostics, then the capability report
                               # reads text.json beside the sources
valrun  file.val ActionName    # run one action, print the execution record
valpack build  ./dir -o app.va
valpack verify app.va
```
