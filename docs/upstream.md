# Bringing changes in from VIctory

VI Causeway follows [Marvisak/VIctory](https://github.com/Marvisak/VIctory). The initial source revision was `1dd7afd`. The first upstream merge includes [`97c78d1`](https://github.com/Marvisak/VIctory/commit/97c78d1087808bd6ce5eabb5d566083a38406a90), including the NVIDIA descriptor heap correction, texture extent validation, and process exit watchdog.

The next merge included pipeline compilation on worker threads, a persistent D3D12 pipeline library, and indirect draws. [Upstream revision 35b054e](https://github.com/Marvisak/VIctory/commit/35b054ec038c11bc8a0de66b1eee764b1ad4f101)

The November 23 evening sync included upstream's generic Intel cache-writeback handling, the Windows 10 static TLS fix, PE parsing tests and size checks for tiled textures and XMA audio. [Upstream revision 062ad14](https://github.com/Marvisak/VIctory/commit/062ad14eae9b89b972d11568f8307746118fab14) · [Windows 10 loader PR](https://github.com/Marvisak/VIctory/pull/28)

The next snapshot included the fix for concurrent writes to guarded pages, the host root-signature test layout, and shader control-flow and arithmetic fixes. [Upstream revision fc0c4dd](https://github.com/Marvisak/VIctory/commit/fc0c4dd331841142ba399b3ba9af2cb912f6695c)

Causeway subsequently picked up Yaekai's patches for null buffer descriptors and the `v_swap_b32` and `v_xnor_b32` shader instructions. Both instructions appeared in world compute shaders on the RTX 3060 Ti. [Upstream PR 57](https://github.com/Marvisak/VIctory/pull/57) · [Upstream PR 58](https://github.com/Marvisak/VIctory/pull/58)

We also tested upstream's wave-size changes against four compute pipelines that the RTX 3060 Ti refused during story loading. All four translated shaders passed DXIL validation. The device accepted the 32-thread pipeline after the change. The three larger pipelines still required 64 lanes and the driver refused them. [Upstream PR 54](https://github.com/Marvisak/VIctory/pull/54)

Two 2560×1440 packed render targets failed to allocate on the same machine. Henrique Bueno's format fallback let the RTX create both in RGBA8 in a native resource test. Causeway also checks copy and resolve compatibility before recording GPU commands. Copies that require conversion between packed guest texels and RGBA8 are still unsupported. [Upstream PR 89](https://github.com/Marvisak/VIctory/pull/89)

Two later upstream changes reduce work during loading and rendering. DirectStorage writes contiguous physical pages through a single mapped view. Lists submitted together share one staging check for each sampled texture until a CPU write invalidates it. [Physical-page writes](https://github.com/Marvisak/VIctory/pull/61) · [Texture staging](https://github.com/Marvisak/VIctory/pull/79)

We tested shader changes against 50 of the 81 distinct shader hashes rejected during the last RTX 3060 Ti story run. The previous release created zero pipelines from this sample. Six pixel shaders passed Microsoft's DXIL validator and created D3D12 pipelines after adding 64-bit texture atomics. Three more passed after adding clamped gradient sampling on cube maps. Each pipeline used the translator's generated vertex shader. This offline test measured pipeline creation. Visual output and story frame time still need an in-game comparison. [64-bit texture atomics](https://github.com/Marvisak/VIctory/pull/48) · [Clamped sampling](https://github.com/Marvisak/VIctory/pull/73) · [Windows cube-map results](https://github.com/Marvisak/VIctory/pull/138#issuecomment-7118644304)

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
