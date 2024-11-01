# Benchmarking diff

The engine used by our diff tool tries to balance execution time with patch
quality. It implements the Myers algorithm with a few heuristics which are also
used by GNU diff to avoid pathological cases.

The original paper can be found here:
- https://link.springer.com/article/10.1007/BF01840446

Currently, not all tricks used by GNU diff are adopted by our implementation.
For instance, GNU diff will isolate lines that only exist in each of the files
and not include them on the diffing process. It also does post-processing of the
edits to produce more cohesive hunks. Both of these combinar should make it
produce better patches for large files which are very different.

Run `cargo build --release` before benchmarking after you make a change!

## How to benchmark

It is recommended that you use the 'hyperfine' tool to run your benchmarks. This
is an example of how to run a comparison with GNU diff:

```
> hyperfine -N -i --warmup 2 --output=pipe 'diff t/huge t/huge.3'
'./target/release/diffutils diff t/huge t/huge.3'
Benchmark 1: diff t/huge t/huge.3
  Time (mean ± σ):     136.3 ms ±   3.0 ms    [User: 88.5 ms, System: 17.9 ms]
  Range (min … max):   131.8 ms … 144.4 ms    21 runs

  Warning: Ignoring non-zero exit code.

Benchmark 2: ./target/release/diffutils diff t/huge t/huge.3
  Time (mean ± σ):      74.4 ms ±   1.0 ms    [User: 47.6 ms, System: 24.9 ms]
  Range (min … max):    72.9 ms …  77.1 ms    41 runs

  Warning: Ignoring non-zero exit code.

Summary
  ./target/release/diffutils diff t/huge t/huge.3 ran
    1.83 ± 0.05 times faster than diff t/huge t/huge.3
>
```

As you can see, you should provide both commands you want to compare on a single
invocation of 'hyperfine'. Each as a single argument, so use quotes. These are
the relevant parameters:

- -N: avoids using a shell as intermediary to run the command
- -i: ignores non-zero exit code, which diff uses to mean files differ
- --warmup 2: 2 runs before measuring, warms up I/O cache for large files
- --output=pipe: disable any potential optimizations based on output destination

## Inputs

Performance will vary based on several factors, the main ones being:

- how large the files being compared are
- how different the files being compared are
- how large and far between sequences of equal lines are

When looking at performance improvements, testing small and large (tens of MBs)
which have few differences, many differences, completely different is important
to cover all of the potential pathological cases.
