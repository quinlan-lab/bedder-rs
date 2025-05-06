Here we show examples of how `bedder` can perform intersections. All examples uses these files:

#### aa.bed

```
chr1 2 23
```

#### bb.bed

```
chr1 8 12
chr1 14 15
chr1 20 30
```

# Reporting Part

For, now, we focus on which part of each interval that is reported. The options for this are:

```
  -p, --a-part <A_PART>
          a-part [default: whole] [possible values: none, part, whole, inverse]
      --b-part <B_PART>
          b-part [default: whole] [possible values: none, part, whole, inverse]
```

Let's start with reporting the *whole* `a` interval if it overlaps and *none* of the `b` interval:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part whole --b-part none
chr1    2       23
```

Now, we report the *part*s of the `a` interval along with the *whole* `b` interval that it overlapped:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part part --b-part whole
chr1    8       12      chr1    8       12
chr1    14      15      chr1    14      15
chr1    20      23      chr1    20      30
```

And now the *part* of `a` and the `part` of `b`:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part part --b-part part
chr1    8       12      chr1    8       12
chr1    14      15      chr1    14      15
chr1    20      23      chr1    20      23
```

We can also report the `inverse`, that is, parts of `a` that do not overlap `b`:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part inverse --b-part none
chr1    2       8
chr1    12      14
chr1    15      20
```

There are other combinations of parameters, some of which are not very helpful!

# Overlap Requirements

The default in bedder is that a single base of overlap is sufficient to report. However we can add constraints to this with these arguments:

```
  -r, --a-requirements <A_REQUIREMENTS>
          a-requirements for overlap. A float value < 1 or a number ending with % will be the fraction (or %) of the interval. An integer will be the number of bases. [default: 1]
  -R, --b-requirements <B_REQUIREMENTS>
          b-requirements for overlap. A float value < 1 or a number ending with % will be the fraction (or %) of the interval. An integer will be the number of bases. [default: 1]
```

Here is the default, requiring a single base of overlap:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part part --b-part none --a-requirements 1
chr1    8       12
chr1    14      15
chr1    20      23
```

We can update that to require at least 3 bases:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part whole --b-part whole --a-requirements 3 --a-mode piece
chr1    2       23      chr1    8       12      chr1    20      30
```

We can also report each a piece:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part part --b-part whole --b-requirements 3 --a-mode piece`
chr1    8       12      chr1    8       12
chr1    20      23      chr1    20      30
```

If we don't specify `--a-mode piece` then it checks across the entire interval so each *part* of `a` is reported even though one of the pieces is not 3 bases:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part part --b-part whole --b-requirements 3`
chr1    8       12      chr1    8       12
chr1    14      15      chr1    14      15
chr1    20      23      chr1    20      30
```

# Python functions

We can output custom columns with python functions. The python function must accept a fragment, part of an overlap, and have a return type of `str`, `int`, `bool` or `float`.
The function must begin with `bedder_`. For example, we can have a function like this that will return the number of `b` intervals overlapping the `a` interval:

```python
def bedder_n_overlapping(fragment) -> int:
    return len(fragment.b)
```

This tells `bedder` that the return type will be an integer. And the user will refer to the function as `py:n_overlapping` (without arguments).

We put this in a file called `example.py` and then run with an argument of `-c py:n_overlapping` as:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part whole --b-part part -P tests/examples/example.py -c 'py:n_overlapping'
chr1    2       23      chr1    8       12      chr1    14      15      chr1    20      23      3
```

Where the final column shows the expected value of *3*.

Another example is that total bases of `b` that overlap an `a` interval:

```python
def bedder_total_b_overlap(fragment) -> int:
    return sum(b.stop - b.start for b in fragment.b)
```

And call as:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part whole --b-part part -P tests/examples/example.py -c 'py:total_b_overlap' 
chr1    2       23      chr1    8       12      chr1    14      15      chr1    20      23      8
```

Note that if we change the `--b-part` to `whole` we get a different value as expected:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part whole --b-part whole -P tests/examples/example.py -c 'py:total_b_overlap' 
chr1    2       23      chr1    8       12      chr1    14      15      chr1    20      30      15
```

and likewise if we change `--a-part` to part:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-part part --b-part whole -P tests/examples/example.py -c 'py:total_b_overlap'
chr1    8       12      aaaa    chr1    8       12      4
chr1    14      15      bbbb    chr1    14      15      1
chr1    20      23      cccc    chr1    20      30      10
```
