
## The layers

- **The structure is declared in `.config/rotproof.toml`, and `rotproof check` fails when the tree differs from it,
  either way.** Delete an unused layer and list it in `absent`; to bring one back, remove it from `absent` and run
  `rotproof create`, which makes only what is missing. Code outside every layer fails too: move it into a layer, or
  list its path in `unchecked` (helper scripts, generated code).{not_layers}
- **Each layer imports only what this table allows, besides itself.** `rotproof check` reads the imports and fails on
  any other. Only direct imports are judged: what the table allows is closed under chaining. Imports built at run
  time are not seen. Each layer's role is in the documentation `rotproof create` wrote into it.

{table}
- **`application` does not import `infrastructure` (dependency inversion).** When a use case needs something external,
  define a port (an interface) in `domain`, and build the real implementation in `handler`. Define a port only when
  `application` calls it, with only the methods it calls.
{ui}