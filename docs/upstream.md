# Bringing changes in from VIctory

VI Causeway follows [Marvisak/VIctory](https://github.com/Marvisak/VIctory). The initial source revision was `1dd7afd`. The first upstream merge includes [`97c78d1`](https://github.com/Marvisak/VIctory/commit/97c78d1087808bd6ce5eabb5d566083a38406a90), including the NVIDIA descriptor heap correction, texture extent validation, and process exit watchdog.

Work in a fresh, single-branch clone of upstream for each refresh. Keep that clone separate from the VI Causeway checkout. Its BCPack table is excluded from the upstream license. VI Causeway reads a user-supplied copy at runtime.

1. Clone upstream main with `--no-tags --single-branch --branch main`. Record the upstream commit ID.
2. Find the table's historical blobs and any new binary files. Write each table blob ID on its own line in a file outside the clone. The original excluded blob is `1e548501764588b06e2ed47aa7fed263cd797f3b`.
3. Run the following command in that disposable clone. These options preserve the original branch topology, author identity, and messages across refreshes.

   ```sh
   git filter-repo --path crates/victory-formats/src/bcpack-tables.bin --invert-paths \
     --strip-blobs-with-ids /path/to/excluded-blobs.txt --replace-refs delete-no-add \
     --prune-empty never --prune-degenerate never --preserve-commit-hashes
   ```

4. Inspect `git rev-list --objects --all` for excluded IDs, excluded filenames, and newly introduced binary content. Check `git fsck --full` and verify that `git cat-file -e <excluded-blob-id>` fails. Record the filtered commit ID from `.git/filter-repo/commit-map`.
5. Fetch the filtered main into a local tracking branch in the VI Causeway checkout with `--no-tags`. Review the difference against the last upstream merge and run the tests for the code being imported. Publish the reviewed VI Causeway branch. Keep the filtered upstream commit as a merge parent when importing a whole upstream revision.

For contributions back to VIctory, use an ordinary GitHub fork of the original project and base the change on its original commit history. [Upstream asset policy](https://github.com/Marvisak/VIctory/blob/main/docs/asset-policy.md)
