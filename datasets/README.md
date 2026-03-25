# Example datasets

This directory contains a deliberately small dataset bundle for miniR examples.

Selection rules:

- Non-R upstream sources only.
- Explicit redistribution terms on the dataset or publisher page.
- No GPL dependency on R packaging.
- No share-alike or ambiguous redistribution terms.

Included datasets:

- `iris.csv` from UCI, CC BY 4.0
- `wine.csv` from UCI, CC BY 4.0
- `auto_mpg.csv` from UCI, CC BY 4.0
- `abalone.csv` from UCI, CC BY 4.0
- `breast_cancer_wisconsin_diagnostic.csv` from UCI, CC BY 4.0
- `penguins.csv` from palmerpenguins, CC0 1.0

All local files in `data/` are normalized CSV copies with header rows added where
needed. Source URLs, licenses, and attribution details are recorded in
`manifest.tsv` and the corresponding `.Rd` files in `man/`.

Refresh the bundle with:

```sh
just update-datasets
```
