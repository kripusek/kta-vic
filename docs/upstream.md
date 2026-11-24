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

mpDom's descriptor changes retain replaced resources across presents and skip draws when a fresh table cannot be allocated. The 3060 Ti still exhausted its 65,536 dynamic slots in both tests. Causeway now remembers failed allocation sizes until a command list returns slots. Overlapping shadow writes also reuse unchanged-page comparisons. On the i5-11600K the copy-back benchmark fell from 234.1 to 10.9 ms for 24 overlapping ranges across 128 MiB. [Descriptor lifetime](https://github.com/Marvisak/VIctory/pull/135) · [Shadow copy-back](https://github.com/Marvisak/VIctory/pull/124)

The final 60-second test includes Dom's changes for reconstructing a scalar buffer descriptor read as data. Four additional vertex shaders passed DXIL validation and created pipelines on the RTX, bringing the earlier 50-shader sample to 13. An intermediate build also changed ordinary scalar SRV loads and rendered the Player screen black. We retained the original loads. In the corrected build the Player capture matched the reference pixel for pixel and all 13 pipelines still passed. [Scalar buffer PR](https://github.com/Marvisak/VIctory/pull/136)

When a bulk read into a host buffer fails, Causeway queries the current region and zero-fills unmapped spans. A 32 MiB synthetic copy with 2 MiB committed took 1.23 ms compared with 27.36 ms using page retries, with identical bytes. Across the first six complete world samples, median CPU copy time fell from 322 to 160 ms per frame between the two builds. The runs also differed in shader coverage, pipeline compilation and draw counts. Overall frame time still exceeds the 33.3 ms target.

The experimental Wave64 branch uses alfycs's implementation for compute groups larger than a native host wave. On the RTX 3060 Ti, three of four saved story compute programs passed DXIL validation and created native pipelines, up from one on the 60-second build. Two large-group compute pipelines were recovered. A fourth program requires a first-lane read in divergent control flow and is still refused. The 13 pipelines in the separate vertex and pixel sample continued to validate and build. The experimental branch still needs a story test. [Wave64 implementation and native results](https://github.com/Marvisak/VIctory/pull/189#issuecomment-7119069414)

The merged Wave64 and renderer changes were tested offline against 177 shaders rejected during story loading. Two passed DXIL validation and created compute pipelines on WARP with a 32-lane host target. An earlier count of 97 included descriptor offsets that could select different lanes. The merged code requires proof that the offsets agree. A separate WARP test confirmed that a GPU-written argument buffer and count could reach an indirect clamp in the wrong resource state, so the new indirect path was held. [Wave64](https://github.com/ifBars/VICauseway/pull/1) and [renderer tests and review](https://github.com/ifBars/VICauseway/pull/2)

Mesh pipeline streams now declare their triangle output topology. A reopened WARP pipeline library served all 21 requests in a synthetic test after the change, up from zero. The effect on story loading has not been measured. [Causeway PR](https://github.com/ifBars/VICauseway/pull/3) and [upstream mesh cache fix](https://github.com/Marvisak/VIctory/pull/352)

Causeway incorporates the pixel barycentrics, PNG capture and optional CLZERO changes from [cat4blep's VIctory branch](https://github.com/Marvisak/VIctory/compare/main...cat4blep:VIctory:main). The recovered story pixel shader created a WARP pipeline with its original vertex shader. A separate WARP draw verified that a null first render target preserves the second output. [Upstream render-target fix](https://github.com/Marvisak/VIctory/pull/450)

Causeway preserves the initial pipeline supplied when creating a command list and uses RGBA16F for RGB9E5 textures that require render-target or unordered access. Sampled-only RGB9E5 textures keep their original format. A WARP clear and readback preserved all four test channels, including color values above one. [Command-list pipeline](https://github.com/Marvisak/VIctory/pull/461) and [RGB9E5 fallback](https://github.com/Marvisak/VIctory/pull/492)

Two previously rejected Wave32 pixel shaders passed the Windows SDK validator after forwarding proven register spills across a mask join. A combined experimental build created both original mesh/pixel pipelines during a bounded RTX 3060 Ti story run. The run reached the first story card and stalled before any visible world geometry.

Watched textures now reuse initialized scratch on each submission thread. GPU writeback snapshots reuse pages across submissions. In a CPU benchmark with 28 expanding declarations on a 174 MB buffer, warm snapshot capture fell from 343.42 to 11.80 ms. Declaring the widest range first increased the median from 13.31 to 17.54 ms. The effect on story frame time needs an in-game measurement. [Watched texture scratch](https://github.com/Marvisak/VIctory/pull/495) and [paged snapshots](https://github.com/Marvisak/VIctory/pull/497)

When a watched range spans several Windows allocations, Causeway now retains dirty-page results from each successful query, including when a later allocation requires a full copy. It also rechecks whether a previously absent mapped block is still absent. Two watched 1 MiB allocations with two dirty 64 KiB pages needed 128 KiB of copying in a CPU test. A watched allocation next to an unwatched one still required a full copy. [Allocation boundaries](https://github.com/Marvisak/VIctory/pull/515) and [uncommitted blocks](https://github.com/Marvisak/VIctory/pull/514)

Causeway now coalesces constant-buffer loads proven to lie in one 16-byte row and reports separate copy footprints for depth and stencil. On 116 previously translated story shaders, two vertex shaders used fewer DXIL loads and 114 retained identical bytecode. WARP readback verified the depth and stencil planes separately. [Constant-buffer loads](https://github.com/Marvisak/VIctory/pull/511) and [depth/stencil footprints](https://github.com/Marvisak/VIctory/pull/504)

Causeway incorporates fuzeofficial's fix for a texture first used as a whole-resource copy source. A WARP test copies CPU-written pixels through two guest textures and checks all 16 destination pixels. [Upstream texture copy fix](https://github.com/Marvisak/VIctory/pull/671)

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
