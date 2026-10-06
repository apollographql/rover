---
category: fix
breaking: false
authors: [dotdat]
---

A masked credential has the same width however long it is

`rover auth whoami`, `rover config whoami` and debug logs mask a credential by keeping its first and last four characters, but they put one `*` in place of every hidden character. An OAuth access token runs to about 2,000 characters, so after `rover auth login` every row of `rover auth whoami`'s table was over 2,000 characters wide. The hidden part is now always eight `*`, for example `eyJr********Qw8A`, for API keys too. A key of one to three characters no longer makes the masking panic in a debug build.
