<!-- marp tests/examples/README.md -o examples.html -->
---
marp: true
theme: default
paginate: true
footer: 'aa.bed:<pre>chr1 2 23</pre>
bb.bed:
<pre>chr1 8 12
<br>chr1 14 15
<br>chr1 20 30</pre>'

style: |
  section {
    padding-bottom: 80px;
  }
  footer {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    height: 90px;
    font-size: 11px;
    background: rgba(255, 255, 255, 0.95);
    display: flex;
    align-items: center;
    padding: 0 10px;
    box-shadow: 0 -1px 3px rgba(0,0,0,0.1);
  }
---

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

---

# Reporting Piece

For, now, we focus on which part of each interval that is reported. The options for this are:

```

  -p, --a-piece <A_PIECE>
          a-piece [default: whole] [possible values: none, part, whole, inverse]
      --b-piece <B_PIECE>
          b-piece [default: whole] [possible values: none, part, whole, inverse]

```

---

Let's start with reporting the *whole* `a` interval if it overlaps and *none* of the `b` interval:

```

$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece whole --b-piece none
chr1    2       23

```

---

Now, we report the *part*s of the `a` interval along with the *whole* `b` interval that it overlapped:

```

$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece part --b-piece whole
chr1    8       12      chr1    8       12
chr1    14      15      chr1    14      15
chr1    20      23      chr1    20      30

```

---

And now the *part* of `a` and the `part` of `b`:

```

$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece part --b-piece part
chr1    8       12      chr1    8       12
chr1    14      15      chr1    14      15
chr1    20      23      chr1    20      23

```

---

We can also report the `inverse`, that is, parts of `a` that do not overlap `b`:

```

$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece inverse --b-piece none
chr1    2       8
chr1    12      14
chr1    15      20

```

---

There are other many combinations of parameters, some of which are not very helpful!

---

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

$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece part --b-piece none --a-requirements 1
chr1    8       12
chr1    14      15
chr1    20      23

```

---

We can update that to require at least 3 bases:

```

$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece whole --b-piece whole --a-requirements 3 --a-mode piece
chr1    2       23      chr1    8       12      chr1    20      30

```

---

We can also report each a piece:

```

$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece part --b-piece whole --b-requirements 3 --a-mode piece`
chr1    8       12      chr1    8       12
chr1    20      23      chr1    20      30

```

---

If we don't specify `--a-mode piece` then it checks across the entire interval so each *part* of `a` is reported even though one of the pieces is not 3 bases:

```

$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece part --b-piece whole --b-requirements 3`
chr1    8       12      chr1    8       12
chr1    14      15      chr1    14      15
chr1    20      23      chr1    20      30

```

---

### Python functions

We can output custom columns with python functions. The python function must accept a fragment, part of an overlap, and have a return type of `str`, `int`, `bool` or `float`.
The function must begin with `bedder_`. For example, we can have a function like this that will return the number of `b` intervals overlapping the `a` interval:

```python
def bedder_n_overlapping(fragment) -> int:
    return len(fragment.b)
```

This tells `bedder` that the return type will be an integer. And the user will refer to the function as `py:n_overlapping` (without arguments).

---

We put this in a file called `example.py` and then run with an argument of `-c py:n_overlapping` as:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece whole --b-piece part -P tests/examples/example.py -c 'py:n_overlapping'
chr1    2       23      chr1    8       12      chr1    14      15      chr1    20      23      3
```

Where the final column shows the expected value of *3*.

---

Another example is that total bases of `b` that overlap an `a` interval:

```python
def bedder_total_b_overlap(fragment) -> int:
    return sum(b.stop - b.start for b in fragment.b)
```

And call as:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece whole --b-piece part -P tests/examples/example.py -c 'py:total_b_overlap' 
chr1    2       23      chr1    8       12      chr1    14      15      chr1    20      23      8
```

---

Note that if we change the `--b-piece` to `whole` we get a different value as expected:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece whole --b-piece whole -P tests/examples/example.py -c 'py:total_b_overlap' 
chr1    2       23      chr1    8       12      chr1    14      15      chr1    20      30      15
```

---

and likewise if we change `--a-piece` to part:

```
$ bedder -a tests/examples/aa.bed -b tests/examples/bb.bed -g tests/examples/fake.fai --a-piece part --b-piece whole -P tests/examples/example.py -c 'py:total_b_overlap'
chr1    8       12      aaaa    chr1    8       12      4
chr1    14      15      bbbb    chr1    14      15      1
chr1    20      23      cccc    chr1    20      30      10
```

---

# VCF and getting to the concrete type

Until now, we have relied on the generic trait methods `chrom`, `start`, `stop` in the `python` functions, but we also have access to the concrete types.
For example, if we know it's a vcf, we can access the underlying variant and only count the depth if the filter is pass:

```python
def bedder_vcf_dp(fragment) -> int:
    """return depth (DP) of passing variant"""
    v = fragment.a.vcf() # get the concrete type
    if v.filter != "PASS": return 0
    dp = v.info("DP") # this is list of length 1 so we return the first element
    return dp[0]
```
