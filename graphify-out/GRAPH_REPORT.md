# Graph Report - cairn  (2026-10-07)

## Corpus Check
- 113 files · ~181,361 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 12 file(s) not represented in the graph (top: (none) 3, .css 3, .woff2 1)

## Summary
- 1611 nodes · 4556 edges · 82 communities (68 shown, 14 thin omitted)
- Extraction: 92% EXTRACTED · 8% INFERRED · 0% AMBIGUOUS · INFERRED: 373 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- Edit Locks & Publishing Tests
- Workspace Discovery & Paths
- Editor & Template UI
- Templates, Idle & Search
- Self-Update Logic
- Export API & File Names
- Config & Server Plumbing
- Template & Download Screens
- Browser Test Harness
- Workspace API Handlers
- Product & Design Principles
- Settings, Timezone & Setup UI
- Update Install Tests
- Restart Handoff
- Path Safety & Resolution
- Page & History API
- Client Formatting Helpers
- Manage API (Rename/Delete)
- App Shell & Leave Guards
- Edit Lock Lifecycle
- PDF Export
- Article Parsing & Links
- Draft Store & Pictures
- Link Rewriting on Move
- Permissions & Export Settings
- Download Tests
- Version History Cleanup
- Printable HTML
- Last-Editor Records
- Folder Management & Trash
- links_and_asset_dirs
- file_urls_need_the_read_key_and_every_
- constant_time_compare
- folderOrganizeSection
- open_initial_workspace
- update_links_elsewhere
- openBulkDownload
- atomic_write_replaces_and_leaves_no_te
- a_page_made_from_a_team_template_has_i
- every_input_has_a_label_and_no_placeho
- dismissedVersion
- publishing_a_picture_copies_it_beside_
- the_timezone_setting_is_saved_and_can_
- invalid_pictures_are_rejected_with_a_c
- milkdown/package.json
- /static/vendor/milkdown.js
- closing_the_terminal_window_gives_back
- DeleteTemplateBody
- normalize_relative
- Packaging (zip, installer, Mac app, Ap
- unknown_zones_are_rejected_and_fall_ba
- View Mode Toggle (As it will look / Sh
- Page Status, Owner and Review Date Met
- Editor View Modes (As it will look / S
- callouts_keep_their_kind_and_nothing_e
- Files on disk layout (shared vs privat
- Earlier versions and automatic cleanup
- Cloud-synced folders unsafe for multi-
- Page Tree Sidebar (sections with count
- Page Actions (Edit, Earlier versions, 
- Sidebar page tree with Templates and R
- Abandoned edit locks and maintainer re
- No central server, coordination throug
- Show times in: Automatic vs Choose a t
- scrollTogether
- Markdown text as single source of trut
- classify
- Issue template config (blank issues di
- a_user_without_write_permission_cannot
- make-icons.sh script
- the_program_carries_the_cairn_logo
- Blue Tonal Palette (#1d4ed8 to #60a5fa
- build-linux.sh script
- Stacked Stones (Cairn) Brand Motif
- build-app.sh script
- cairn

## God Nodes (most connected - your core abstractions)
1. `AppState` - 126 edges
2. `h()` - 109 edges
3. `Root` - 82 edges
4. `mountEditor()` - 76 edges
5. `button()` - 55 edges
6. `toast()` - 50 edges
7. `blocking()` - 50 edges
8. `post()` - 48 edges
9. `clear()` - 48 edges
10. `errorText()` - 45 edges

## Surprising Connections (you probably didn't know these)
- `Packaging (zip, installer, Mac app, AppImage)` --semantically_similar_to--> `Release workflow`  [INFERRED] [semantically similar]
  docs/building.md → .github/workflows/release.yml
- `invalid_metadata_never_makes_a_page_unreadable()` --calls--> `parse_front_matter()`  [INFERRED]
  tests/content.rs → src/article.rs
- `rendered()` --calls--> `render()`  [INFERRED]
  tests/content.rs → src/article.rs
- `folder_exports_never_follow_folder_links()` --calls--> `folder_files()`  [INFERRED]
  tests/export.rs → src/export/archive.rs
- `folder_exports_skip_system_hidden_and_temporary_files()` --calls--> `folder_files()`  [INFERRED]
  tests/export.rs → src/export/archive.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Serverless multi-user coordination via files in the shared folder** — readme_edit_locks, readme_earlier_versions, readme_unsaved_drafts_and_conflicts, readme_plain_markdown_files, contributing_fsutil_rs_write_atomic [INFERRED 0.85]
- **Cairn design system principles** — design_plain_handbook, design_one_blue_rule, design_inter_only_rule, design_scale_with_text_rule, design_three_column_layout [EXTRACTED 0.95]
- **Local security and signed update chain** — security_loopback_threat_model, contributing_release_signing, readme_signed_self_update, contributing_paths_rs_resolve [INFERRED 0.75]
- **Safe concurrent editing mechanisms** — docs_storage_and_concurrency_edit_locks, docs_storage_and_concurrency_conflict_detection, docs_storage_and_concurrency_publishing_sequence, docs_storage_and_concurrency_private_drafts, docs_storage_and_concurrency_atomic_primitives [EXTRACTED 0.95]
- **Release pipeline: tag, build per OS, sign, publish** — _github_workflows_release, _github_workflows_release_minisign_signing, docs_building_packaging, docs_configuration_updates, docs_manual_two_computer_checklist_release_update_checks [INFERRED 0.85]
- **Local-only web security layers** — docs_architecture_local_program, docs_architecture_local_web_security, docs_architecture_paths_gate, docs_architecture_visual_editing_markdown_truth [INFERRED 0.75]
- **Page Export Formats** — docs_screenshots_download_pdf_export, docs_screenshots_download_markdown_zip_export, docs_screenshots_download_markdown_md_export [EXTRACTED 1.00]
- **Safe page editing flow: lock, edit with preview, review diff, publish** — docs_screenshots_editor_page_edit_lock, docs_screenshots_editor_split_markdown_editor, docs_screenshots_editor_publish_with_review [INFERRED 0.85]
- **Home screen ways to find a page (browse, search, recent)** — docs_screenshots_home_sidebar_navigation, docs_screenshots_home_search_all_pages, docs_screenshots_home_folders_list, docs_screenshots_home_recently_changed [INFERRED 0.85]
- **Three-column page layout: nav tree, content with metadata/actions, on-page TOC** — docs_screenshots_page_sidebar_page_tree, docs_screenshots_page_page_metadata, docs_screenshots_page_page_actions, docs_screenshots_page_on_this_page_toc [INFERRED 0.85]
- **Page editing session: lock, edit visually, publish** — docs_screenshots_visual_page_edit_lock, docs_screenshots_visual_wysiwyg_editor, docs_screenshots_visual_publish_changes_flow [INFERRED 0.85]

## Communities (82 total, 14 thin omitted)

### Community 0 - "Edit Locks & Publishing Tests"
Cohesion: 0.06
Nodes (47): TestWorkspace, workspace(), a_page_created_meanwhile_by_someone_else_is_a_conflict(), a_user_can_reclaim_their_own_crashed_lock_on_the_same_computer(), an_older_picture_is_not_deleted_when_its_reference_is_removed(), call(), every_failure_point_keeps_the_previous_article_and_leaves_no_dangling_picture(), has_draft() (+39 more)

### Community 1 - "Workspace Discovery & Paths"
Cohesion: 0.06
Nodes (54): create_new_with(), read_optional(), display_path(), adopt_existing(), boundary_of(), canonical(), check_only_expected_entries(), Claim (+46 more)

### Community 2 - "Editor & Template UI"
Cohesion: 0.08
Nodes (66): api(), confirmDialog(), formatDateTime(), formatSize(), formatTime(), lineDiff(), close(), openDialog() (+58 more)

### Community 3 - "Templates, Idle & Search"
Cohesion: 0.05
Nodes (42): allowed_typos(), ceil_boundary(), edit_distance(), Entry, excerpt(), excerpt_highlights_terms(), floor_boundary(), FolderEntry (+34 more)

### Community 4 - "Self-Update Logic"
Cohesion: 0.06
Nodes (50): Version, a_mac_app_run_from_downloads_does_not_self_update(), agent(), appimage_path(), asset_name(), check(), CHECK_TIMEOUT, current_version() (+42 more)

### Community 5 - "Export API & File Names"
Cohesion: 0.07
Nodes (31): download(), ExportJobs, Format, Md, Pdf, Zip, Job, job_cancel() (+23 more)

### Community 6 - "Config & Server Plumbing"
Cohesion: 0.06
Nodes (28): AppConfig, CONFIG_FILE, defaults_are_valid_and_release_must_follow_warning(), load(), roundtrip(), save(), CairnError, BadRequest (+20 more)

### Community 7 - "Template & Download Screens"
Cohesion: 0.14
Nodes (39): breadcrumbsNav(), emptyState(), formatDay(), get(), icon(), linkButton(), menuButton(), relativeTime() (+31 more)

### Community 8 - "Browser Test Harness"
Cohesion: 0.07
Nodes (28): @playwright/test, addPicture(), apiGet(), BIN, editorText(), failRequests(), holdRequests(), leavingAsksFirst() (+20 more)

### Community 9 - "Workspace API Handlers"
Cohesion: 0.10
Nodes (33): breadcrumbs(), choose_folder(), DiscardBody, discover(), edit_activity(), idle_config(), init_workspace(), InitBody (+25 more)

### Community 10 - "Product & Design Principles"
Cohesion: 0.08
Nodes (28): Inter font (SIL Open Font License), /static/js/app.js entry module, assets/index.html SPA shell, Changelog (0.6.0 to 1.0.0), Cairn 1.0: Mac and Linux support, Contributing guide, src/fsutil.rs write_atomic / create_new_with, src/paths.rs Root::resolve (sole path gateway) (+20 more)

### Community 11 - "Settings, Timezone & Setup UI"
Cohesion: 0.18
Nodes (35): button(), clear(), fieldError(), h(), openPicture(), showWorking(), systemTimeZone(), whileBusy() (+27 more)

### Community 12 - "Update Install Tests"
Cohesion: 0.11
Nodes (15): options(), zip_files(), ZipBuilder, a_signed_newer_release_is_found_downloaded_and_unpacked(), asset(), connect_proxy(), downloads_not_signed_by_the_right_key_are_refused(), FakeGitHub (+7 more)

### Community 13 - "Restart Handoff"
Cohesion: 0.11
Nodes (25): now_secs(), begin_install(), CHECK_EVERY, check_now(), exe_path(), FIRST_CHECK_AFTER, Handoff, HANDOFF_FILE (+17 more)

### Community 14 - "Path Safety & Resolution"
Cohesion: 0.10
Nodes (18): folder_files(), is_hidden_or_temp(), page_files(), pages_in(), is_link_like(), pages_under(), component_eq(), is_within() (+10 more)

### Community 15 - "Page & History API"
Cohesion: 0.18
Nodes (30): title_from_filename(), validate_article_path(), all_folders(), all_pages(), blocking(), close_workspace(), ContentBody, draft_discard() (+22 more)

### Community 16 - "Client Formatting Helpers"
Cohesion: 0.09
Nodes (29): ApiError, dayNumber(), DIFF_WORD, diffRow(), diffRows(), diffSummary(), diffView(), dispositionName() (+21 more)

### Community 17 - "Manage API (Rename/Delete)"
Cohesion: 0.20
Nodes (24): parent_of(), all_pages(), delete_folder(), delete_page(), DeletePageBody, ensure_not_editing(), finish_move(), FolderBody (+16 more)

### Community 18 - "App Shell & Leave Guards"
Cohesion: 0.16
Nodes (29): app, boot(), buildTree(), closeNav(), currentLocation(), LOADING_TEXT, makeContext(), navLink() (+21 more)

### Community 19 - "Edit Lock Lifecycle"
Cohesion: 0.22
Nodes (26): Acquire, Acquired, HeldBy, computer_name(), heartbeat(), HEARTBEAT_SECS, held_error(), Identity (+18 more)

### Community 20 - "PDF Export"
Cohesion: 0.12
Nodes (18): app_executable(), candidate_paths(), failed(), file_url(), file_url_encodes_spaces_and_keeps_drive(), find_browsers(), hide_window(), is_whole_pdf() (+10 more)

### Community 21 - "Article Parsing & Links"
Cohesion: 0.12
Nodes (21): ArticleMeta, bad_values_are_warnings_not_errors(), CALLOUT_CLASSES, hex_val(), invalid_front_matter_still_returns_body(), is_image_ext(), is_valid_date(), parse_front_matter() (+13 more)

### Community 22 - "Draft Store & Pictures"
Cohesion: 0.18
Nodes (8): article_key(), Draft, DraftStore, mem_key(), MemEntry, SaveOutcome, StagedImage, validate_image_name()

### Community 23 - "Link Rewriting on Move"
Cohesion: 0.14
Nodes (16): is_local_link(), a_moved_page_keeps_its_own_links_and_pictures(), find_dest(), links_to_a_moved_page_are_updated_and_nothing_else_changes(), may_link_to(), nothing_to_change_returns_none(), page(), page_map_moves_its_pictures_too() (+8 more)

### Community 24 - "Permissions & Export Settings"
Cohesion: 0.14
Nodes (9): cannot_edit_reason(), check_write_permission(), edit_start(), EditStartBody, print_settings(), folder_denied(), AppState, OpenWorkspace (+1 more)

### Community 25 - "Download Tests"
Cohesion: 0.19
Nodes (15): a_folder_downloads_as_a_zip_through_a_job(), a_page_downloads_as_markdown_with_its_file_name(), export_routes_need_the_token(), folder_exports_never_follow_folder_links(), folder_exports_skip_system_hidden_and_temporary_files(), folder_jobs_refuse_cairns_own_folder(), get(), link_folder() (+7 more)

### Community 26 - "Version History Cleanup"
Cohesion: 0.19
Nodes (13): Author, author_file(), Cleanup, history_dir(), list_versions(), old_versions_go_but_the_newest_three_stay(), parse_stamp(), prune_all() (+5 more)

### Community 27 - "Printable HTML"
Cohesion: 0.14
Nodes (11): asset_text(), body_html(), CSP, embedded_pictures(), PICTURE_PLACEHOLDER, printable_html(), PrintInput, status_label() (+3 more)

### Community 28 - "Last-Editor Records"
Cohesion: 0.22
Nodes (17): edited_path(), EditRecord, last_edit(), LastEdit, By, Outside, Unknown, lookup() (+9 more)

### Community 29 - "Folder Management & Trash"
Cohesion: 0.24
Nodes (17): managed_folder(), not_moved(), relocate_page(), require_writable_dir(), assets_dir_rel(), begin(), CONTENT_DIR, ITEM_FILE (+9 more)

### Community 30 - "links_and_asset_dirs"
Cohesion: 0.15
Nodes (17): can_write_dir(), delete_article(), FailPoint, AfterAssets, AfterHistory, AfterReplace, BeforeReplace, MAX_ARTICLE_BYTES (+9 more)

### Community 31 - "file_urls_need_the_read_key_and_every_"
Cohesion: 0.16
Nodes (9): api_requires_the_per_launch_token(), file_urls_need_the_read_key_and_every_response_has_a_strict_csp(), lan_address(), local(), PORT, send(), state(), the_server_listens_on_loopback_only() (+1 more)

### Community 32 - "constant_time_compare"
Cohesion: 0.17
Nodes (9): constant_time_eq(), cross_site(), CSP, deny(), guard(), host_ok(), origin_ok(), query_param() (+1 more)

### Community 33 - "folderOrganizeSection"
Cohesion: 0.37
Nodes (18): formDialog(), toast(), deletedRow(), deletedView(), deleteFolder(), deletePage(), folderChoices(), folderOrganizeSection() (+10 more)

### Community 34 - "open_initial_workspace"
Cohesion: 0.27
Nodes (15): become_new_version(), cmd_init(), cmd_locks(), cmd_locks_list(), cmd_locks_release(), cmd_run(), fail(), HANDOFF_BIND_WAIT (+7 more)

### Community 35 - "update_links_elsewhere"
Cohesion: 0.22
Nodes (16): sha256_hex(), folder_path(), HeldLocks, HeldLocks<'a>, history_path(), LinkUpdate, move_folder(), move_history() (+8 more)

### Community 36 - "openBulkDownload"
Cohesion: 0.36
Nodes (17): announce(), banner(), downloadFile(), errorText(), guardAction(), choiceCards(), chosen(), doneMessage() (+9 more)

### Community 37 - "atomic_write_replaces_and_leaves_no_te"
Cohesion: 0.15
Nodes (8): keygen(), main(), sign(), atomic_write_replaces_and_leaves_no_temp(), is_case_only_rename(), same_entry(), temp_path_for(), write_temp_beside()

### Community 38 - "a_page_made_from_a_team_template_has_i"
Cohesion: 0.22
Nodes (11): a_page_made_from_a_team_template_has_its_placeholders_filled(), call(), deleting_a_template_keeps_it_restorable(), MEETING, new_templates_get_a_free_path_inside_templates(), only_templates_can_be_deleted_through_the_template_route(), PORT, state() (+3 more)

### Community 39 - "every_input_has_a_label_and_no_placeho"
Cohesion: 0.25
Nodes (15): asset(), buttons_always_carry_visible_words(), colors(), every_input_has_a_label_and_no_placeholder_stands_in_for_one(), every_theme_meets_the_contrast_targets(), focus_outlines_are_never_removed(), html_is_only_inserted_when_constant_or_server_sanitized(), js_files() (+7 more)

### Community 40 - "dismissedVersion"
Cohesion: 0.27
Nodes (15): appendChildren(), checkNow(), dismiss(), dismissedVersion(), downloadLink(), listeners, refresh(), releaseNotes() (+7 more)

### Community 41 - "publishing_a_picture_copies_it_beside_"
Cohesion: 0.19
Nodes (11): encode_path(), relative_link(), image_link_for(), a_link_to_a_heading_on_the_same_page_works_and_a_missing_one_is_marked(), copy_dir(), markdown_features_render_and_scripts_never_do(), missing_local_targets_are_flagged_and_existing_ones_link_inside_the_app(), PAGE (+3 more)

### Community 42 - "the_timezone_setting_is_saved_and_can_"
Cohesion: 0.18
Nodes (8): today(), new_pages_are_dated_in_the_chosen_zone(), one_moment_reads_correctly_in_each_zone(), PORT, post(), secs(), state(), the_timezone_setting_is_saved_and_can_go_back_to_automatic()

### Community 43 - "invalid_pictures_are_rejected_with_a_c"
Cohesion: 0.22
Nodes (9): asset_file_name(), damaged(), ImageInfo, ImageLimits, looks_like_markup(), naming_is_stable_and_slugged(), sniff(), validate_image() (+1 more)

### Community 44 - "milkdown/package.json"
Cohesion: 0.14
Nodes (12): esbuild, @milkdown/kit, description, devDependencies, esbuild, @milkdown/kit, license, name (+4 more)

### Community 45 - "/static/vendor/milkdown.js"
Cohesion: 0.29
Nodes (10): encodePath(), calloutLook(), createVisualEditor(), decodeSafe(), emptyTitles(), isExternal(), load(), resolveRelative() (+2 more)

### Community 46 - "closing_the_terminal_window_gives_back"
Cohesion: 0.29
Nodes (7): closing_the_terminal_window_gives_back_edit_locks(), post(), Running, sigterm_gives_back_edit_locks(), start(), stopping_gives_back_locks(), wait_for_exit()

### Community 47 - "DeleteTemplateBody"
Cohesion: 0.33
Nodes (6): create(), delete(), deleted(), DeleteTemplateBody, list(), NewTemplateBody

### Community 48 - "normalize_relative"
Cohesion: 0.29
Nodes (11): sniff_mime(), is_system_path(), normalize_relative(), create_folder(), CreateFolderBody, draft_file(), folder_for(), file_response() (+3 more)

### Community 49 - "Packaging (zip, installer, Mac app, Ap"
Cohesion: 0.25
Nodes (11): CI workflow, Release workflow, Minisign release signing, Building Cairn doc, cargo build --release --locked, Packaging (zip, installer, Mac app, AppImage), Signed self-update mechanism, Getting started doc (+3 more)

### Community 50 - "unknown_zones_are_rejected_and_fall_ba"
Cohesion: 0.25
Nodes (5): format_moment(), offset_label(), user_zone(), zone_name(), unknown_zones_are_rejected_and_fall_back_safely()

### Community 51 - "View Mode Toggle (As it will look / Sh"
Cohesion: 0.33
Nodes (7): Editor Screenshot (Editing a Page), Plain-Language Formatting Toolbar, Office Handbook (sample wiki), Page Tree Sidebar (sections, Templates, Recently deleted), Paste or Drag-in Pictures, Split Markdown Editor with Live Preview, View Mode Toggle (As it will look / Show formatting codes / Codes only)

### Community 52 - "Page Status, Owner and Review Date Met"
Cohesion: 0.28
Nodes (9): Home Screen Screenshot, Folders List with Page Counts, Page Status, Owner and Review Date Metadata, Recently Changed Pages Feed, Recently Deleted (Trash), Search All Pages, Shared Network Folder Storage (UNC path), Sidebar Folder Tree Navigation (+1 more)

### Community 53 - "Editor View Modes (As it will look / S"
Cohesion: 0.31
Nodes (6): Callout / Tip Block, Formatting Toolbar (Bold, Italic, Lists, Link, Picture, Table, Code), Office Handbook Sample Wiki, Page Tree Sidebar (sections, page counts, Templates, Recently deleted), Visual Editor Screenshot (Editing: Wi-Fi keeps disconnecting), WYSIWYG Page Editor (As it will look mode)

### Community 54 - "callouts_keep_their_kind_and_nothing_e"
Cohesion: 0.28
Nodes (8): callouts_keep_their_kind_and_nothing_else(), extract_title(), md_options(), plain_text(), render_notes(), sanitize(), misspelled_searches_get_a_suggestion(), summarize()

### Community 55 - "Files on disk layout (shared vs privat"
Cohesion: 0.32
Nodes (8): Files on disk layout (shared vs private), Pictures and .assets folders, Recently deleted (trash and restore), CAIRN_HOME environment variable, config.json settings file, Edit locks with heartbeat, Idle time-out and automatic unlock, Renaming, moving, deleting under locks

### Community 56 - "Earlier versions and automatic cleanup"
Cohesion: 0.29
Nodes (8): Earlier versions and automatic cleanup, Configuration doc, Cairn command line (init, locks), Workspace format schema version 1, Open or create a documentation folder, Storage and concurrency doc, Race-safe workspace setup journal, Workspace marker (shared-docs.json)

### Community 57 - "Cloud-synced folders unsafe for multi-"
Cohesion: 0.29
Nodes (7): Publishing changes workflow, Who last edited a page (_system/edited), Cloud-synced folders unsafe for multi-user editing, SHA-256 conflict detection before publish, Private drafts on local computer, Safe publishing sequence (publish.rs), Storage probe

### Community 58 - "Page Tree Sidebar (sections with count"
Cohesion: 0.29
Nodes (8): Download This Page Dialog (screenshot), Markdown File (.md) Export, Markdown with Pictures (.zip) Export, Office Handbook (sample wiki), On This Page Table of Contents, Page Metadata Bar (status, owner, review date, last edited, tags), Page Tree Sidebar (sections with counts, Templates, Recently deleted), PDF Document Export

### Community 59 - "Page Actions (Edit, Earlier versions, "
Cohesion: 0.29
Nodes (8): Breadcrumb Navigation, On This Page Table of Contents, Page Actions (Edit, Earlier versions, Download, More actions), Page Metadata (status, owner, reviewed date, tags), Page View Screenshot (Office Handbook), Global Search Bar, Sidebar Page Tree (sections with counts), Templates and Recently Deleted

### Community 60 - "Sidebar page tree with Templates and R"
Cohesion: 0.29
Nodes (8): Built-in templates (Blank page, Step-by-step guide), Templates belong to the documentation folder, Make a team copy action, Plain-language, non-technical UI design, Sidebar page tree with Templates and Recently deleted, Team templates (How-to guide, Incident report), Templates page screenshot (Cairn, Office Handbook), Use for a new page action

### Community 61 - "Abandoned edit locks and maintainer re"
Cohesion: 0.29
Nodes (6): Docs problem issue template, PDF export via headless browser, Downloads (PDF, Markdown zip), Troubleshooting doc, Abandoned edit locks and maintainer release, PDF unavailable fallback to print view

### Community 62 - "No central server, coordination throug"
Cohesion: 0.33
Nodes (4): Architecture doc, Local Cairn program (axum on 127.0.0.1), paths.rs path-validation gate, In-memory search index

### Community 63 - "Show times in: Automatic vs Choose a t"
Cohesion: 0.33
Nodes (7): Edit lock idle prompt and unlock timing settings, Live preview of time format (Sep 30, 2026, 7:45 AM GMT+8), Sidebar page tree navigation (Office Handbook), Region and City near you dropdown pickers, Show times in: Automatic vs Choose a timezone radio options, Time and timezone settings screenshot (Office Handbook), Times stored in UTC and shown in viewer's timezone

### Community 64 - "scrollTogether"
Cohesion: 0.53
Nodes (5): headingLines(), lineTops(), mapPosition(), scrollTogether(), measure()

### Community 65 - "Markdown text as single source of trut"
Cohesion: 0.33
Nodes (5): Writing pages doc, Page details front matter, Page templates with {{fields}}, Visual editor (as it will look, codes, codes only), Template front matter

### Community 66 - "classify"
Cohesion: 0.33
Nodes (6): classify(), LinkKind, Anchor, External, Local, Unsafe

### Community 67 - "Issue template config (blank issues di"
Cohesion: 0.40
Nodes (5): Bug report issue template, Issue template config (blank issues disabled), Feature request issue template, Hard to use (accessibility) issue template, Question issue template

## Knowledge Gaps
- **191 isolated node(s):** `cairn`, `app`, `ROUTES`, `LOADING_TEXT`, `SYSTEM` (+186 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 370 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **14 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppState` connect `Permissions & Export Settings` to `constant_time_compare`, `Edit Locks & Publishing Tests`, `open_initial_workspace`, `Export API & File Names`, `Config & Server Plumbing`, `a_page_made_from_a_team_template_has_i`, `Workspace API Handlers`, `the_timezone_setting_is_saved_and_can_`, `Restart Handoff`, `Page & History API`, `normalize_relative`, `Manage API (Rename/Delete)`, `DeleteTemplateBody`, `Edit Lock Lifecycle`, `Draft Store & Pictures`, `Download Tests`, `file_urls_need_the_read_key_and_every_`?**
  _High betweenness centrality (0.073) - this node is a cross-community bridge._
- **Are the 10 inferred relationships involving `mountEditor()` (e.g. with `applyDetails()` and `changed()`) actually correct?**
  _`mountEditor()` has 10 INFERRED edges - model-reasoned connections that need verification._
- **What connects `cairn`, `app`, `ROUTES` to the rest of the system?**
  _191 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Edit Locks & Publishing Tests` be split into smaller, more focused modules?**
  _Cohesion score 0.05583972719522592 - nodes in this community are weakly interconnected._
- **Why does `Root` connect `Path Safety & Resolution` to `Edit Locks & Publishing Tests`, `Workspace Discovery & Paths`, `open_initial_workspace`, `update_links_elsewhere`, `Templates, Idle & Search`, `Config & Server Plumbing`, `Workspace API Handlers`, `publishing_a_picture_copies_it_beside_`, `Update Install Tests`, `Page & History API`, `Edit Lock Lifecycle`, `Article Parsing & Links`, `Permissions & Export Settings`, `Version History Cleanup`, `Printable HTML`, `Last-Editor Records`, `Folder Management & Trash`, `links_and_asset_dirs`?**
  _High betweenness centrality (0.031) - this node is a cross-community bridge._
- **Should `Workspace Discovery & Paths` be split into smaller, more focused modules?**
  _Cohesion score 0.05754475703324808 - nodes in this community are weakly interconnected._
- **Why does `CairnError` connect `Config & Server Plumbing` to `Self-Update Logic`, `atomic_write_replaces_and_leaves_no_te`, `invalid_pictures_are_rejected_with_a_c`, `Path Safety & Resolution`, `Edit Lock Lifecycle`, `PDF Export`, `Permissions & Export Settings`, `Folder Management & Trash`, `links_and_asset_dirs`?**
  _High betweenness centrality (0.017) - this node is a cross-community bridge._