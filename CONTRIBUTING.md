# Contributing to diffutils

Hi! Welcome to uutils/diffutils, and thanks for wanting to contribute!

This project follows the shared conventions of the [uutils](https://github.com/uutils)
organization. Before opening a pull request, please read:

- Our **[Review Guidelines](https://uutils.github.io/reviews/)** — what we expect
  from a pull request and how reviews are carried out.
- Our community's [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md), if present.

Finally, feel free to join our [Discord](https://discord.gg/wQVJbvJ)!

> [!WARNING]
> uutils is original code and cannot contain any code from GNU or other
> strongly-licensed (GPL/LGPL) implementations. We **cannot** accept changes
> based on the GNU source code, and you **must not link** to it either. You may
> look at permissively-licensed implementations (MIT/BSD) and read the GNU
> *manuals* — never the GNU *source*.

## In short

- Discuss non-trivial changes in an issue **before** writing the code.
- Keep pull requests **small, self-contained, and descriptively titled**
  (e.g. `diffutils: fix ...`).
- Make sure CI passes: tests are green, `rustfmt` is satisfied, and there are
  no `clippy` warnings.
- Add tests for new behavior; don't let coverage regress.
- Write small, atomic commits annotated with the component you touched.

See the [Review Guidelines](https://uutils.github.io/reviews/) for the full
details.
