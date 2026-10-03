- **`ui` follows atomic design (`pages` > `templates` > `organisms` > `molecules` > `atoms`), and holds only its
  levels.** Besides direction, the table limits what each level knows: `atoms` and `molecules` know neither `domain`
  nor `application` and take plain values; `organisms` and `templates` do not call `application` and take actions as
  callbacks; only `pages` call use cases; `ui` does not know `infrastructure`. What the whole UI shares goes in the
  lowest level that may know what it knows: a part that renders nothing (a design value, a hook, a provider of a
  theme) is an atom when it knows no project concept, and code beside the levels fails `rotproof check`.
