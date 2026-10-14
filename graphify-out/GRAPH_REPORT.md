# Graph Report - cairn  (2026-10-14)

## Corpus Check
- 66 files · ~222,407 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 2399 nodes · 6412 edges · 114 communities (92 shown, 22 thin omitted)
- Extraction: 92% EXTRACTED · 8% INFERRED · 0% AMBIGUOUS · INFERRED: 517 edges (avg confidence: 0.86)
- Token cost: 153,746 input · 0 output

## Community Hubs (Navigation)
- Backup & Export API
- Pages & Folders API
- Export Archive & HTML
- Workspace Discovery & Init
- Editing & Manage Tests
- Editor UI Helpers
- Server Routing & Update Tests
- Windows Tray Icon
- Browser Test Harness
- Backup Create & Restore
- Page & Home Views
- Self-Update Logic
- Backup Manifest Checks
- Download Dialogs & Health UI
- PDF Export & Shutdown
- Draft Store & UI Quality
- Core Client & Tab Token
- Settings & Setup UI
- Article Rendering & Callouts
- Archive Enumeration
- Folder Manage API
- WinHTTP & Update Failures
- Update API Handlers
- Backup & Health Features
- Troubleshooting & Getting Started
- Release Notes & Authoring Docs
- App State & Server Core
- CLI Entry Point
- Held Locks & Moves
- App Shell & Routing
- Link Check Tests
- Edit Locks
- Link Rewriting
- Time Zones
- Storage Concepts Docs
- Mark As Reviewed
- Templates Module
- Search Filter Tests
- Update Proxy Tests
- Version History
- Dialogs & Delete UI
- Link Checker
- Security Tests
- CLI & Safe Publishing Docs
- Contributing & Browser Tests
- Search Ranking
- paths.rs: Component
- Template Tests & Config
- Tabs, Tray & Path Docs
- Release Process Docs
- Last-Editor Records
- Request Security
- Backup Estimate & Progress
- Filesystem Helpers
- Update UI
- Product Design Docs
- Trash & Folder Rules
- Health Tests
- Design System Docs
- Proxy & Backup Docs
- Image Handling
- Health Scan
- Publish Pipeline
- Milkdown Build Tooling
- Error Types
- Search Index
- Update Failure Messages
- Index Resilience Tests
- Visual Editor
- Front Matter Parsing
- Config Loading
- Content & Link Tests
- Templates API
- Path & File Serving
- Backup Download Tests
- Editor Screenshot
- Home Screenshot
- Visual Editor Screenshot
- Idle Detection
- Restore Identity
- Claude Notes & Graph
- Download Screenshot
- Page Screenshot
- Templates Screenshot
- Update Fetching
- timezone.png: Edit lock idle prompt and unlock t
- scrollsync.js: scrollsync.js
- release_sign.rs: release_sign.rs
- search.rs: SortBy
- update.rs: Platform
- article.rs: LinkKind
- publish.rs: FailPoint
- make-icons.sh: make-icons.sh
- app_icon.rs: app_icon.rs
- favicon.svg: Blue Tonal Palette (#1d4ed8 to #60a
- build-linux.sh: build-linux.sh
- cairn.png: Cairn App Icon (Linux)
- build-app.sh: build-app.sh
- SECURITY.md: Loopback-only local server threat m
- Cargo.toml: cairn

## God Nodes (most connected - your core abstractions)
1. `AppState` - 147 edges
2. `h()` - 124 edges
3. `Root` - 108 edges
4. `mountEditor()` - 76 edges
5. `button()` - 60 edges
6. `blocking()` - 55 edges
7. `post()` - 54 edges
8. `toast()` - 52 edges
9. `errorText()` - 50 edges
10. `clear()` - 48 edges

## Surprising Connections (you probably didn't know these)
- `Packaging (zip, installer, Mac app, AppImage)` --semantically_similar_to--> `Release workflow (release.yml)`  [INFERRED] [semantically similar]
  docs/building.md → .github/workflows/release.yml
- `the_handover_file_is_used_once_and_only_while_fresh()` --calls--> `take_handoff()`  [INFERRED]
  tests/update.rs → src/server/api_update.rs
- `invalid_pictures_are_rejected_with_a_clear_reason()` --calls--> `validate_image()`  [INFERRED]
  tests/content.rs → src/images.rs
- `Review due 365 days after last_reviewed` --conceptually_related_to--> `Documentation health screen`  [INFERRED]
  docs/configuration.md → CHANGELOG.md
- `Updates: daily check, proxy order, Update and restart, Go back` --conceptually_related_to--> `Cairn updates itself with signed downloads`  [INFERRED]
  docs/configuration.md → CHANGELOG.md

## Import Cycles
- 2-file cycle: `src/server/api.rs -> src/server/api_export.rs -> src/server/api.rs`

## Hyperedges (group relationships)
- **Cairn design system principles** — design_plain_handbook, design_one_blue_rule, design_inter_only_rule, design_scale_with_text_rule, design_three_column_layout [EXTRACTED 0.95]
- **Safe concurrent editing mechanisms** — docs_storage_and_concurrency_edit_locks, docs_storage_and_concurrency_conflict_detection, docs_storage_and_concurrency_publishing_sequence, docs_storage_and_concurrency_private_drafts, docs_storage_and_concurrency_atomic_primitives [EXTRACTED 0.95]
- **Backup, verify and restore flow around the cairn-backup.json manifest** — docs_backup_and_restore_create, docs_backup_and_restore_verify, docs_backup_and_restore_restore, docs_backup_and_restore_manifest, docs_backup_and_restore_free_space [EXTRACTED 1.00]
- **Page Export Formats** — docs_screenshots_download_pdf_export, docs_screenshots_download_markdown_zip_export, docs_screenshots_download_markdown_md_export [EXTRACTED 1.00]
- **Release pipeline: build on three systems, sign, publish, refresh site** — github_workflows_release_windows, github_workflows_release_macos, github_workflows_release_linux, github_workflows_release_publish, github_workflows_release_minisign, github_workflows_release_website [EXTRACTED 1.00]
- **Local security and signed update chain** — security_loopback_threat_model [INFERRED 0.75]
- **Safe page editing flow: lock, edit with preview, review diff, publish** — docs_screenshots_editor_page_edit_lock, docs_screenshots_editor_split_markdown_editor, docs_screenshots_editor_publish_with_review [INFERRED 0.85]
- **Three-column page layout: nav tree, content with metadata/actions, on-page TOC** — docs_screenshots_page_sidebar_page_tree, docs_screenshots_page_page_metadata, docs_screenshots_page_page_actions, docs_screenshots_page_on_this_page_toc [INFERRED 0.85]
- **Page editing session: lock, edit visually, publish** — docs_screenshots_visual_page_edit_lock, docs_screenshots_visual_wysiwyg_editor, docs_screenshots_visual_publish_changes_flow [INFERRED 0.85]
- **Home screen ways to find a page (browse, search, recent)** — docs_screenshots_home_sidebar_navigation, docs_screenshots_home_search_all_pages, docs_screenshots_home_folders_list, docs_screenshots_home_recently_changed [INFERRED 0.85]
- **Safe concurrent editing: locks, drafts, safe publish, conflict view** — docs_architecture_locks, docs_architecture_drafts, docs_architecture_publish, docs_authoring_conflict, docs_authoring_idle [INFERRED 0.85]

## Communities (114 total, 22 thin omitted)

### Community 0 - "Backup & Export API"
Cohesion: 0.04
Nodes (60): a_crash_in_the_work_fails_the_job_and_frees_the_next_download(), a_finished_job_is_kept_for_ten_minutes_after_it_ends(), BACKUP_FILE_PREFIX, Built, check_result(), download(), download_headers(), ExportJobs (+52 more)

### Community 1 - "Pages & Folders API"
Cohesion: 0.09
Nodes (64): title_from_filename(), read_optional(), all_folders(), all_pages(), blocking(), breadcrumbs(), choose_folder(), close_workspace() (+56 more)

### Community 2 - "Export Archive & HTML"
Cohesion: 0.06
Nodes (55): folder_files(), zip_files(), asset_text(), attribute(), body_html(), converted_links(), CSP, embedded_pictures() (+47 more)

### Community 3 - "Workspace Discovery & Init"
Cohesion: 0.05
Nodes (52): display_path(), adopt_existing(), assign_new_identity(), boundary_of(), canonical(), check_only_expected_entries(), Claim, Finished (+44 more)

### Community 4 - "Editing & Manage Tests"
Cohesion: 0.06
Nodes (47): TestWorkspace, workspace(), a_page_created_meanwhile_by_someone_else_is_a_conflict(), a_user_can_reclaim_their_own_crashed_lock_on_the_same_computer(), an_older_picture_is_not_deleted_when_its_reference_is_removed(), call(), every_failure_point_keeps_the_previous_article_and_leaves_no_dangling_picture(), has_draft() (+39 more)

### Community 5 - "Editor UI Helpers"
Cohesion: 0.08
Nodes (56): api(), formatSize(), formatTime(), IS_MAC, lineDiff(), todayYmd(), toolbarKeys(), folderOf() (+48 more)

### Community 6 - "Server Routing & Update Tests"
Cohesion: 0.07
Nodes (24): ApiError, Assets, bind_loopback(), ErrorBody, index_html(), serve_asset(), static_file(), a_proxy_that_wants_a_sign_in_is_reported_not_bypassed() (+16 more)

### Community 7 - "Windows Tray Icon"
Cohesion: 0.05
Nodes (34): add_icon(), add_icon_with_retries(), ADD_PAUSE, ADD_TRIES, anchor(), CLASS_NAME, CLOSING, Context (+26 more)

### Community 8 - "Browser Test Harness"
Cohesion: 0.06
Nodes (29): @playwright/test, addPicture(), apiGet(), BIN, editorText(), failRequests(), holdRequests(), leavingAsksFirst() (+21 more)

### Community 9 - "Backup Create & Restore"
Cohesion: 0.10
Nodes (40): create_backup(), restore(), read_marker(), a_backup_holds_everything_needed_to_recover_and_no_locks(), a_backup_never_follows_a_file_swapped_for_a_link(), a_backup_never_overwrites_an_existing_file(), a_backup_restores_into_a_new_folder_exactly(), a_backup_that_could_not_be_restored_is_not_kept() (+32 more)

### Community 10 - "Page & Home Views"
Cohesion: 0.12
Nodes (44): appendChildren(), breadcrumbsNav(), dayNumber(), emptyState(), formatDateTime(), formatDay(), get(), icon() (+36 more)

### Community 11 - "Self-Update Logic"
Cohesion: 0.07
Nodes (40): can_write_dir(), a_mac_app_run_from_downloads_does_not_self_update(), ACCEPT, appimage_path(), asset_name(), check(), CHECK_TIMEOUT, current_version() (+32 more)

### Community 12 - "Backup Manifest Checks"
Cohesion: 0.09
Nodes (35): add_streamed(), BACKUP_FORMAT, BACKUP_FORMAT_VERSION, bad(), check_manifest(), check_path(), CHUNK, create_out() (+27 more)

### Community 13 - "Download Dialogs & Health UI"
Cohesion: 0.15
Nodes (41): announce(), banner(), downloadFile(), guardAction(), h(), href, showWorking(), choiceCards() (+33 more)

### Community 14 - "PDF Export & Shutdown"
Cohesion: 0.08
Nodes (28): app_executable(), candidate_paths(), failed(), file_url(), file_url_encodes_spaces_and_keeps_drive(), find_browsers(), hide_window(), is_whole_pdf() (+20 more)

### Community 15 - "Draft Store & UI Quality"
Cohesion: 0.11
Nodes (23): article_key(), Draft, DraftStore, mem_key(), MemEntry, SaveOutcome, StagedImage, validate_image_name() (+15 more)

### Community 16 - "Core Client & Tab Token"
Cohesion: 0.08
Nodes (35): answerOtherTabs(), ApiError, askOtherTabs(), bootstrapToken(), DIFF_WORD, diffRow(), diffRows(), diffSummary() (+27 more)

### Community 17 - "Settings & Setup UI"
Cohesion: 0.20
Nodes (39): button(), clear(), enablePictureZoom(), errorText(), fieldError(), formatIsoDateTime(), post(), toast() (+31 more)

### Community 18 - "Article Rendering & Callouts"
Cohesion: 0.09
Nodes (33): anchor_is_missing(), CALLOUT_CLASSES, callouts_keep_their_kind_and_nothing_else(), classify(), collect_headings(), extract_title(), hex_val(), is_image_ext() (+25 more)

### Community 19 - "Archive Enumeration"
Cohesion: 0.08
Nodes (13): Entry, enumerate(), enumerate_pages(), is_hidden_or_temp(), is_page(), options(), page_files(), pages_in() (+5 more)

### Community 20 - "Folder Manage API"
Cohesion: 0.17
Nodes (28): parent_of(), check_write_permission(), all_pages(), delete_folder(), delete_page(), DeletePageBody, ensure_not_editing(), finish_move() (+20 more)

### Community 21 - "WinHTTP & Update Failures"
Cohesion: 0.10
Nodes (27): failures_are_told_apart(), NetFail, Incomplete, ProxyAuth, RateLimited, Status, Timeout, Tls (+19 more)

### Community 22 - "Update API Handlers"
Cohesion: 0.11
Nodes (24): now_secs(), begin_install(), CHECK_EVERY, check_now(), exe_path(), FIRST_CHECK_AFTER, Handoff, HANDOFF_FILE (+16 more)

### Community 23 - "Backup & Health Features"
Cohesion: 0.08
Nodes (33): cairn backup create/verify and cairn restore, Back up workspace, Check documentation (broken-link check), Documentation health screen, Free-space check before backup and restore, Mark as reviewed today, PDFs keep the meaning of links, Warning before a PDF changes links (+25 more)

### Community 24 - "Troubleshooting & Getting Started"
Cohesion: 0.07
Nodes (29): Bug report issue template, Issue template config (blank issues disabled), Docs problem issue template, Feature request issue template, Hard to use (accessibility) issue template, Question issue template, Per-user Windows installer (no admin password), Cairn updates itself with signed downloads (+21 more)

### Community 25 - "Release Notes & Authoring Docs"
Cohesion: 0.07
Nodes (31): Callout boxes (Note, Tip, Important, Warning, Caution), Cairn for Mac and Linux, Downloads (PDF, Markdown), Keeps you on the page while saving, Links keep working after rename or move, cairn restore --new-identity, Proxy server setting, Recently deleted and Restore (+23 more)

### Community 26 - "App State & Server Core"
Cohesion: 0.11
Nodes (10): cannot_edit_reason(), folder_denied(), health(), page_from_template(), today_for(), AppState, draft_store_for(), EditSession (+2 more)

### Community 27 - "CLI Entry Point"
Cohesion: 0.18
Nodes (24): become_new_version(), cmd_backup(), cmd_init(), cmd_locks(), cmd_locks_list(), cmd_locks_release(), cmd_restore(), cmd_run() (+16 more)

### Community 28 - "Held Locks & Moves"
Cohesion: 0.17
Nodes (21): sha256_hex(), LOCKS_DIR, HeldLocks, HeldLocks<'a>, history_path(), LinkUpdate, mark_reviewed(), move_folder() (+13 more)

### Community 29 - "App Shell & Routing"
Cohesion: 0.17
Nodes (28): app, boot(), buildTree(), closeNav(), currentLocation(), LOADING_TEXT, makeContext(), navLink() (+20 more)

### Community 30 - "Link Check Tests"
Cohesion: 0.15
Nodes (21): check_workspace(), a_check_cannot_start_while_another_job_is_running_and_says_why(), a_check_runs_as_a_job_with_progress_and_hands_over_its_result_once(), a_check_that_missed_an_unreadable_folder_says_it_is_partial(), a_link_through_a_shortcut_folder_is_reported_not_followed(), a_page_that_cannot_be_read_makes_the_check_incomplete(), a_renamed_page_shows_up_on_the_next_check_and_clears_when_it_returns(), BROKEN (+13 more)

### Community 31 - "Edit Locks"
Cohesion: 0.21
Nodes (25): Acquire, Acquired, HeldBy, computer_name(), heartbeat(), HEARTBEAT_SECS, held_error(), Identity (+17 more)

### Community 32 - "Link Rewriting"
Cohesion: 0.12
Nodes (19): encode_path(), is_local_link(), a_moved_page_keeps_its_own_links_and_pictures(), find_dest(), links_to_a_moved_page_are_updated_and_nothing_else_changes(), may_link_to(), nothing_to_change_returns_none(), page() (+11 more)

### Community 33 - "Time Zones"
Cohesion: 0.11
Nodes (13): format_moment(), offset_label(), today(), user_zone(), zone_name(), new_pages_are_dated_in_the_chosen_zone(), one_moment_reads_correctly_in_each_zone(), PORT (+5 more)

### Community 34 - "Storage Concepts Docs"
Cohesion: 0.11
Nodes (25): Team templates, drafts.rs: private per-user unsaved changes, locks.rs: edit locks and heartbeats, Private files on each computer (config.json, drafts/), publish.rs: safe publishing sequence, docs/authoring.md (Writing pages), Earlier versions and optional cleanup, One editor per page: Edit this page locks it for others (+17 more)

### Community 35 - "Mark As Reviewed"
Cohesion: 0.11
Nodes (13): a_page_someone_is_editing_is_not_touched(), a_page_that_cannot_be_edited_automatically_is_left_alone(), a_read_only_documentation_folder_cannot_be_changed_by_a_review(), already_reviewed_today_changes_nothing(), DAY, marking_reviewed_keeps_the_credit_the_status_and_an_earlier_version(), page(), PORT (+5 more)

### Community 36 - "Templates Module"
Cohesion: 0.12
Nodes (21): BUILTIN, builtin_raw(), deleted(), DeletedTemplate, describe(), front_matter_block(), instantiate(), instantiate_strips_template_keys_and_fills_placeholders() (+13 more)

### Community 37 - "Search Filter Tests"
Cohesion: 0.24
Nodes (20): a_filter_without_words_lists_the_pages_that_match_it(), a_folder_filter_includes_nested_folders_but_not_look_alikes(), an_exact_title_comes_first_even_against_pages_that_say_it_more(), equal_matches_come_in_a_predictable_order(), filters_are_applied_before_the_result_limit(), fresh(), get(), indexed() (+12 more)

### Community 38 - "Update Proxy Tests"
Cohesion: 0.14
Nodes (13): a_proxy_from_settings_beats_everything_else(), an_empty_variable_counts_as_unset(), environment_variables_beat_windows_settings(), LAST_NETWORK, NetworkInfo, PREFER_WINHTTP, proxy(), PROXY_VARIABLES (+5 more)

### Community 39 - "Version History"
Cohesion: 0.18
Nodes (14): Author, author_file(), Cleanup, history_dir(), list_versions(), old_versions_go_but_the_newest_three_stay(), parse_stamp(), prune_all() (+6 more)

### Community 40 - "Dialogs & Delete UI"
Cohesion: 0.29
Nodes (21): confirmDialog(), formDialog(), openDialog(), conflict(), restoreDialog(), deletedRow(), deletedView(), deleteFolder() (+13 more)

### Community 41 - "Link Checker"
Cohesion: 0.22
Nodes (18): check_fragment(), check_page(), check_workspace_with(), CheckProgress, CheckReport, Finding, Headings, into_cairns_folder() (+10 more)

### Community 42 - "Security Tests"
Cohesion: 0.15
Nodes (11): a_user_without_write_permission_cannot_publish_into_a_protected_folder(), api_requires_the_per_launch_token(), DenyWrite, file_urls_need_the_read_key_and_every_response_has_a_strict_csp(), lan_address(), local(), PORT, send() (+3 more)

### Community 43 - "CLI & Safe Publishing Docs"
Cohesion: 0.10
Nodes (19): cairn init and cairn locks list/release, Safe publishing and one editor per page, Release 0.1.0 (first working release, Windows), Command line: --no-browser, --port, --workspace, init, locks, cairn init from the command line, Created layout: shared-docs.json, Guides/, _system/, Open or create a documentation folder, Crash recovery steps (+11 more)

### Community 44 - "Contributing & Browser Tests"
Cohesion: 0.11
Nodes (20): Optional Playwright browser tests (tests/e2e), Build and run (cargo run, CAIRN_HOME dev setup), CAIRN_HOME environment variable, Conventional commit messages, Debug builds read assets/ from disk; release embeds them, h() and button() DOM helpers in assets/js/core.js, Prerequisites: Rust 1.88, MSVC Build Tools, Git, Rust test suite (tests/*.rs) (+12 more)

### Community 45 - "Search Ranking"
Cohesion: 0.16
Nodes (18): allowed_typos(), ceil_boundary(), edit_distance(), Entry, excerpt(), excerpt_highlights_terms(), floor_boundary(), FolderEntry (+10 more)

### Community 46 - "paths.rs: Component"
Cohesion: 0.14
Nodes (11): folder_path(), managed_folder(), validate_folder_name(), component_eq(), is_within(), join_relative_link(), reject(), RESERVED_NAMES (+3 more)

### Community 47 - "Template Tests & Config"
Cohesion: 0.19
Nodes (11): a_page_made_from_a_team_template_has_its_placeholders_filled(), call(), deleting_a_template_keeps_it_restorable(), MEETING, new_templates_get_a_free_path_inside_templates(), only_templates_can_be_deleted_through_the_template_route(), PORT, state() (+3 more)

### Community 48 - "Tabs, Tray & Path Docs"
Cohesion: 0.13
Nodes (15): Second tab connects by itself, Windows tray icon instead of console window, Release 1.2.0, axum/tokio HTTP server on 127.0.0.1, Browser interface (plain HTML/CSS/JS modules in assets/), links.rs and manage.rs: rewriting links on rename/move, Local program (cairn binary, src/main.rs), Milkdown editor engine (vendored, pre-built) (+7 more)

### Community 49 - "Release Process Docs"
Cohesion: 0.18
Nodes (18): App icons generated from assets/favicon.svg, Trying an update locally (CAIRN_UPDATE_API, CAIRN_UPDATE_KEY), Release by pushing a v* tag, Release signing key (PUBLIC_KEY in src/update.rs, MINISIGN_SECRET_KEY secret), Website rebuild via Vercel deploy hook, Updates installed only if signed with the built-in key, Release workflow (release.yml), Linux AppImage and tar.gz packaging (+10 more)

### Community 50 - "Last-Editor Records"
Cohesion: 0.22
Nodes (16): edited_path(), EditRecord, last_edit(), LastEdit, By, Outside, Unknown, lookup() (+8 more)

### Community 51 - "Request Security"
Cohesion: 0.18
Nodes (9): constant_time_eq(), cross_site(), CSP, deny(), guard(), host_ok(), origin_ok(), query_param() (+1 more)

### Community 52 - "Backup Estimate & Progress"
Cohesion: 0.26
Nodes (18): attempts(), backup_estimate(), changed_files(), check_backup_space(), create(), create_checked(), Progress, Summary (+10 more)

### Community 53 - "Filesystem Helpers"
Cohesion: 0.16
Nodes (11): atomic_write_replaces_and_leaves_no_temp(), create_new_with(), is_case_only_rename(), same_entry(), temp_path_for(), write_atomic(), write_temp_beside(), dir_of() (+3 more)

### Community 54 - "Update UI"
Cohesion: 0.23
Nodes (16): IS_WINDOWS, CONNECTION_NAMES, connectionLine(), dismiss(), dismissedVersion(), downloadLink(), listeners, refresh() (+8 more)

### Community 55 - "Product Design Docs"
Cohesion: 0.12
Nodes (15): Docs-site look: page tree, centred column, On this page, See who last edited a page, Release 0.7.0, UI guidelines: visible words, 44px targets, plain language, tokens.css colours, Content-Security-Policy and ammonia sanitizing, Who last edited a page, Accessibility: WCAG 2.2 AA, keyboard, high contrast, Brand commitments: name Cairn, favicon, words on buttons, Inter (+7 more)

### Community 56 - "Trash & Folder Rules"
Cohesion: 0.28
Nodes (14): require_writable_dir(), assets_dir_rel(), begin(), CONTENT_DIR, ITEM_FILE, last_segment(), list(), read_item() (+6 more)

### Community 57 - "Health Tests"
Cohesion: 0.29
Nodes (12): a_scan_that_could_not_read_everything_says_so(), counts_match_the_list_and_only_pages_with_issues_are_listed(), each_page_is_classified_by_what_needs_attention(), fresh(), index(), issues_of(), page(), PORT (+4 more)

### Community 58 - "Design System Docs"
Cohesion: 0.17
Nodes (11): Inter font (SIL Open Font License), /static/js/app.js entry module, assets/index.html SPA shell, Cairn design system (DESIGN.md), Design tokens (tokens.css), Editor toolbar (signature component), Three-column docs layout (tree, article, On this page), Themes: light, dark, high contrast (+3 more)

### Community 59 - "Proxy & Backup Docs"
Cohesion: 0.13
Nodes (13): Updates through NTLM/Kerberos proxies; fixed proxy order, Release 1.0.1, Backup consistency check (not a snapshot), Proxy order: Settings, environment variables, Windows settings, direct, Where Cairn can update itself (not from a network drive), Updates: daily check, proxy order, Update and restart, Go back, Numbered steps 1-15: locks, conflicts, drafts, pictures, permissions, security, Downloads and backups on the share (+5 more)

### Community 60 - "Image Handling"
Cohesion: 0.21
Nodes (10): asset_file_name(), damaged(), ImageInfo, ImageLimits, looks_like_markup(), naming_is_stable_and_slugged(), sniff(), sniff_mime() (+2 more)

### Community 61 - "Health Scan"
Cohesion: 0.18
Nodes (6): ISSUES, issues_for(), PageHealth, Report, PageSummary, set_last_edit()

### Community 62 - "Publish Pipeline"
Cohesion: 0.23
Nodes (10): delete_article(), MAX_ARTICLE_BYTES, publish(), PublishOutcome, Conflict, Published, PublishRequest, remove_created() (+2 more)

### Community 63 - "Milkdown Build Tooling"
Cohesion: 0.14
Nodes (12): esbuild, @milkdown/kit, description, devDependencies, esbuild, @milkdown/kit, license, name (+4 more)

### Community 64 - "Error Types"
Cohesion: 0.18
Nodes (12): CairnError, BadRequest, Conflict, InvalidImage, Io, Locked, NotFound, PathRejected (+4 more)

### Community 65 - "Search Index"
Cohesion: 0.24
Nodes (5): is_link_like(), all_folders(), list_folder(), SearchIndex, skip_dir_name()

### Community 66 - "Update Failure Messages"
Cohesion: 0.16
Nodes (12): each_failure_has_its_own_message(), Connect, NotFound, ProxySource, Direct, Environment, Manual, WindowsSystem (+4 more)

### Community 67 - "Index Resilience Tests"
Cohesion: 0.32
Nodes (8): a_folder_that_cannot_be_read_keeps_its_pages_and_is_reported(), a_page_that_cannot_be_read_stays_listed_and_is_reported(), a_review_falls_due_a_year_after_the_last_one(), a_same_size_change_a_moment_later_is_still_noticed(), deleted_pages_leave_the_index_and_a_clean_scan_reports_nothing(), metadata_problems_travel_with_the_page_into_lists(), page(), refreshed()

### Community 68 - "Visual Editor"
Cohesion: 0.29
Nodes (10): encodePath(), calloutLook(), createVisualEditor(), decodeSafe(), emptyTitles(), isExternal(), load(), resolveRelative() (+2 more)

### Community 69 - "Front Matter Parsing"
Cohesion: 0.18
Nodes (11): ArticleMeta, bad_values_are_warnings_not_errors(), invalid_front_matter_still_returns_body(), is_valid_date(), parse_front_matter(), review_due_on(), scalar_to_string(), set_last_reviewed() (+3 more)

### Community 70 - "Config Loading"
Cohesion: 0.30
Nodes (6): AppConfig, CONFIG_FILE, defaults_are_valid_and_release_must_follow_warning(), load(), roundtrip(), save()

### Community 71 - "Content & Link Tests"
Cohesion: 0.26
Nodes (9): a_link_to_a_heading_on_the_same_page_works_and_a_missing_one_is_marked(), copy_dir(), invalid_pictures_are_rejected_with_a_clear_reason(), markdown_features_render_and_scripts_never_do(), missing_local_targets_are_flagged_and_existing_ones_link_inside_the_app(), PAGE, publishing_a_picture_copies_it_beside_the_page_with_a_working_relative_link(), remote_images_are_never_fetched() (+1 more)

### Community 72 - "Templates API"
Cohesion: 0.38
Nodes (6): create(), delete(), deleted(), DeleteTemplateBody, list(), NewTemplateBody

### Community 73 - "Path & File Serving"
Cohesion: 0.33
Nodes (9): is_system_path(), normalize_relative(), create_folder(), CreateFolderBody, draft_file(), file_response(), folder_rel(), not_found() (+1 more)

### Community 74 - "Backup Download Tests"
Cohesion: 0.24
Nodes (5): a_backup_downloads_through_a_job_then_its_file_is_removed(), request(), send(), state(), waiting_backups()

### Community 75 - "Editor Screenshot"
Cohesion: 0.33
Nodes (7): Editor Screenshot (Editing a Page), Plain-Language Formatting Toolbar, Office Handbook (sample wiki), Page Tree Sidebar (sections, Templates, Recently deleted), Paste or Drag-in Pictures, Split Markdown Editor with Live Preview, View Mode Toggle (As it will look / Show formatting codes / Codes only)

### Community 76 - "Home Screenshot"
Cohesion: 0.28
Nodes (9): Home Screen Screenshot, Folders List with Page Counts, Page Status, Owner and Review Date Metadata, Recently Changed Pages Feed, Recently Deleted (Trash), Search All Pages, Shared Network Folder Storage (UNC path), Sidebar Folder Tree Navigation (+1 more)

### Community 77 - "Visual Editor Screenshot"
Cohesion: 0.31
Nodes (6): Callout / Tip Block, Formatting Toolbar (Bold, Italic, Lists, Link, Picture, Table, Code), Office Handbook Sample Wiki, Page Tree Sidebar (sections, page counts, Templates, Recently deleted), Visual Editor Screenshot (Editing: Wi-Fi keeps disconnecting), WYSIWYG Page Editor (As it will look mode)

### Community 78 - "Idle Detection"
Cohesion: 0.25
Nodes (6): evaluate(), IdleState, Active, Release, Warning, M

### Community 79 - "Restore Identity"
Cohesion: 0.36
Nodes (8): Identity, Keep, New, restore_checked(), restore_space_needed(), restore_with(), restore_with_identity(), Restored

### Community 80 - "Claude Notes & Graph"
Cohesion: 0.25
Nodes (5): CLAUDE.md (notes for Claude), Caveman response style in chat, graphify-out/graph.json, graphify-out/GRAPH_REPORT.md, Graphify knowledge graph (graphify-out/)

### Community 81 - "Download Screenshot"
Cohesion: 0.29
Nodes (8): Download This Page Dialog (screenshot), Markdown File (.md) Export, Markdown with Pictures (.zip) Export, Office Handbook (sample wiki), On This Page Table of Contents, Page Metadata Bar (status, owner, review date, last edited, tags), Page Tree Sidebar (sections with counts, Templates, Recently deleted), PDF Document Export

### Community 82 - "Page Screenshot"
Cohesion: 0.29
Nodes (8): Breadcrumb Navigation, On This Page Table of Contents, Page Actions (Edit, Earlier versions, Download, More actions), Page Metadata (status, owner, reviewed date, tags), Page View Screenshot (Office Handbook), Global Search Bar, Sidebar Page Tree (sections with counts), Templates and Recently Deleted

### Community 83 - "Templates Screenshot"
Cohesion: 0.29
Nodes (8): Built-in templates (Blank page, Step-by-step guide), Templates belong to the documentation folder, Make a team copy action, Plain-language, non-technical UI design, Sidebar page tree with Templates and Recently deleted, Team templates (How-to guide, Incident report), Templates page screenshot (Cairn, Office Handbook), Use for a new page action

### Community 84 - "Update Fetching"
Cohesion: 0.36
Nodes (5): agent(), fetch(), fetch_ureq(), fetch_winhttp(), UpdateSource

### Community 85 - "timezone.png: Edit lock idle prompt and unlock t"
Cohesion: 0.33
Nodes (7): Edit lock idle prompt and unlock timing settings, Live preview of time format (Sep 30, 2026, 7:45 AM GMT+8), Sidebar page tree navigation (Office Handbook), Region and City near you dropdown pickers, Show times in: Automatic vs Choose a timezone radio options, Time and timezone settings screenshot (Office Handbook), Times stored in UTC and shown in viewer's timezone

### Community 86 - "scrollsync.js: scrollsync.js"
Cohesion: 0.53
Nodes (5): headingLines(), lineTops(), mapPosition(), scrollTogether(), measure()

### Community 87 - "release_sign.rs: release_sign.rs"
Cohesion: 0.47
Nodes (3): keygen(), main(), sign()

### Community 88 - "search.rs: SortBy"
Cohesion: 0.33
Nodes (4): SortBy, Best, Title, Updated

### Community 89 - "update.rs: Platform"
Cohesion: 0.33
Nodes (5): Platform, Linux, LinuxAppImage, Mac, Windows

### Community 91 - "article.rs: LinkKind"
Cohesion: 0.40
Nodes (5): LinkKind, Anchor, External, Local, Unsafe

### Community 92 - "publish.rs: FailPoint"
Cohesion: 0.40
Nodes (5): FailPoint, AfterAssets, AfterHistory, AfterReplace, BeforeReplace

## Knowledge Gaps
- **299 isolated node(s):** `Author`, `Assets`, `ErrorBody`, `META_KEYS`, `TEMPLATE_KEYS` (+294 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 581 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **22 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppState` connect `App State & Server Core` to `Backup & Export API`, `Pages & Folders API`, `Export Archive & HTML`, `Editing & Manage Tests`, `Server Routing & Update Tests`, `Windows Tray Icon`, `Folder Manage API`, `Update API Handlers`, `CLI Entry Point`, `Link Check Tests`, `Time Zones`, `Mark As Reviewed`, `Search Filter Tests`, `Security Tests`, `Template Tests & Config`, `Request Security`, `Health Tests`, `Templates API`, `Path & File Serving`, `Backup Download Tests`, `api_health.rs: arc`?**
  _High betweenness centrality (0.059) - this node is a cross-community bridge._
- **Are the 10 inferred relationships involving `mountEditor()` (e.g. with `applyDetails()` and `changed()`) actually correct?**
  _`mountEditor()` has 10 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Author`, `Assets`, `ErrorBody` to the rest of the system?**
  _299 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Backup & Export API` be split into smaller, more focused modules?**
  _Cohesion score 0.044356435643564354 - nodes in this community are weakly interconnected._
- **Why does `Root` connect `Held Locks & Moves` to `Pages & Folders API`, `Export Archive & HTML`, `Workspace Discovery & Init`, `Editing & Manage Tests`, `Server Routing & Update Tests`, `Backup Create & Restore`, `Backup Manifest Checks`, `Article Rendering & Callouts`, `Archive Enumeration`, `Folder Manage API`, `App State & Server Core`, `CLI Entry Point`, `Link Check Tests`, `Edit Locks`, `Templates Module`, `Version History`, `Link Checker`, `paths.rs: Component`, `Last-Editor Records`, `Backup Estimate & Progress`, `Trash & Folder Rules`, `Publish Pipeline`, `Search Index`, `Content & Link Tests`?**
  _High betweenness centrality (0.044) - this node is a cross-community bridge._
- **Should `Pages & Folders API` be split into smaller, more focused modules?**
  _Cohesion score 0.09181784524250278 - nodes in this community are weakly interconnected._
- **Why does `OpenWorkspace` connect `Folder Manage API` to `Backup & Export API`, `Pages & Folders API`, `Search Index`, `Workspace Discovery & Init`, `Server Routing & Update Tests`, `App State & Server Core`, `Held Locks & Moves`?**
  _High betweenness centrality (0.016) - this node is a cross-community bridge._