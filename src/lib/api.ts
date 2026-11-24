import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

export interface GitInfo {
  path: string;
  version: string;
  source?: "system" | "bundled";
}

export type Head = { kind: "branch"; name: string } | { kind: "unborn"; name: string } | { kind: "detached"; id: string };

export type Change = "added" | "modified" | "deleted" | "typeChange";

export interface Entry {
  path: string;
  kind: "tracked" | "untracked" | "conflicted";
  index: Change | null;
  worktree: Change | null;
}

export interface Status {
  root: string;
  head: Head;
  entries: Entry[];
}

export interface Opened {
  id: number;
  root: string;
  /** Graph rows, the working-copy row included. */
  rows: number;
  /** Bumped each time the graph is rebuilt: cached rows are stale. */
  graphVersion: number;
}

export type RefLabel =
  | { kind: "head" }
  | { kind: "branch"; name: string; current: boolean }
  | { kind: "remote"; name: string }
  | { kind: "tag"; name: string }
  | { kind: "stash"; name: string }
  | { kind: "proposal"; name: string };

export interface Segment {
  from: number;
  to: number;
  part: "top" | "bottom" | "through";
  /** Plan previews dash the planned result and fade what it replaces; absent is solid. */
  stroke?: "dashed" | "faded";
}

export interface RowLayout {
  node: number;
  segments: Segment[];
  overflow: boolean;
}

export interface Row {
  id: string;
  shortId: string;
  subject: string;
  author: string;
  /** Author time, seconds since the Unix epoch. */
  time: number;
  labels: RefLabel[];
  wip: boolean;
  /** On a plan preview's planned commit: what the plan does to it ("reworded"…). */
  after?: string | null;
  /** On a plan preview: a commit the plan replaces. */
  faded?: boolean;
  /** Stash index (`stash@{n}`) when this row is a stash. */
  stash?: number;
  /** Proposal id when this row is an active agent proposal ghost commit. */
  proposal?: string | null;
  layout: RowLayout;
}

export interface Person {
  name: string;
  email: string;
  /** Seconds since the Unix epoch. */
  time: number;
}

export interface FileChange {
  path: string;
  change: Change;
}

export interface CommitDetail {
  id: string;
  shortId: string;
  subject: string;
  body: string;
  author: Person;
  committer: Person;
  parents: string[];
  /** Against the first parent, no rename detection, sorted by path. */
  files: FileChange[];
}

export interface DiffLine {
  kind: "context" | "added" | "removed";
  old: number | null;
  new: number | null;
  text: string;
}

export interface Hunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: DiffLine[];
}

export interface BinarySide {
  size: number;
  lfs: boolean;
  oid?: string | null;
  hydrated?: boolean;
}

export interface BinaryMeta {
  before: BinarySide | null;
  after: BinarySide | null;
}

export interface FileDiff {
  path: string;
  binary: boolean;
  hunks: Hunk[];
  binaryMeta?: BinaryMeta | null;
}

export const LFS_AUTO_FETCH_MAX_BYTES = 25 * 1024 * 1024;

export interface Branch {
  /** Short name, e.g. `feature/login`. */
  name: string;
  current: boolean;
  /** e.g. `origin/main`. */
  upstream: string | null;
  ahead: number;
  behind: number;
}

export interface Remote {
  name: string;
  /** e.g. `origin/main`. */
  branches: string[];
}

export interface Branches {
  local: Branch[];
  remotes: Remote[];
  /** When the repo last fetched, seconds since the Unix epoch; `null` if never. */
  fetched: number | null;
}

export interface OpEntry {
  id: number;
  /** Seconds since the Unix epoch. */
  time: number;
  action: string;
  undone: boolean;
  /** Files a destructive action (a discard) changed. */
  paths: string[];
}

export interface Reverted {
  opened: Opened;
  /** The action undone or redone, e.g. `Stage a.txt`. */
  action: string;
}

export type ConflictKind = "unityYaml" | "code" | "binary";

export interface Conflict {
  path: string;
  kind: ConflictKind;
  /** Staged since the merge stopped. */
  resolved: boolean;
}

export interface VerifyItem {
  fileId: string;
  objectName: string;
  property: string;
  base: string | null;
  ours: string | null;
  theirs: string | null;
  merged: string | null;
  confirmed: boolean;
}

export interface AutoMergedItem {
  fileId: string;
  objectName: string;
  property: string;
  side: string;
  value: string | null;
}

export interface UnityMergeFile {
  path: string;
  verify: VerifyItem[];
  autoMerged: AutoMergedItem[];
}

export type MetaIssueKind = "missingMeta" | "orphanMeta" | "guidCollision" | "caseMismatch";

export interface MetaIssue {
  kind: MetaIssueKind;
  path: string;
  detail: string;
}

export interface UnityInfo {
  isUnity: boolean;
  projectRoot: string | null;
  editorOpen: boolean;
  editorVersion: string | null;
  smartMergeConfigured: boolean;
  smartMergeToolPath: string | null;
  detectedEditors: string[];
}

export interface LfsInfo {
  installed: boolean;
  version: string | null;
  trackedPatterns: string[];
  supportsLocks: boolean;
  locksUnsupportedReason: string | null;
}

export interface LfsLock {
  id: string;
  path: string;
  owner: string;
  lockedAt: string;
  ours: boolean;
}

export interface TagInfo {
  name: string;
  target: string;
  shortTarget: string;
  annotated: boolean;
  message: string | null;
  time: number;
}

export interface FileCommit {
  id: string;
  shortId: string;
  subject: string;
  author: string;
  time: number;
  change: string;
}

export interface BlameLine {
  lineNo: number;
  commit: string;
  shortCommit: string;
  author: string;
  time: number;
  summary: string;
  text: string;
}

export interface OpLogStats {
  entryCount: number;
  totalBytes: number;
  prunableCount: number;
  reclaimableBytes: number;
}

export interface ReflogEntry {
  id: string;
  shortId: string;
  selector: string;
  action: string;
  time: number;
}

export interface UnityPropChange {
  property: string;
  before: string | null;
  after: string | null;
  beforeDisplay: string | null;
  afterDisplay: string | null;
}

export interface UnityObjectChange {
  fileId: string;
  objectPath: string;
  component: string;
  kind: "added" | "removed" | "modified";
  properties: UnityPropChange[];
}

/** A file version as the backend names it (`image::Version`). */
export type FileVersion =
  | "head"
  | "index"
  | "worktree"
  | "yours"
  | "theirs"
  | { commit: string }
  | { parent: string };

export type ModelKind = "fbx" | "obj" | "gltf" | "glb" | "prefab" | "asset";

/** Keys for one property of the object at `path`, relative to the Animator. */
export interface ClipTrack {
  path: string;
  property: "position" | "quaternion" | "scale";
  times: number[];
  values: number[];
}

export type ClipSource =
  | { type: "curves"; duration: number; looping: boolean; tracks: ClipTrack[] }
  | { type: "model"; path: string; take: string };

/** A clip the Animator on `node` can play. */
export interface SceneClip {
  node: number;
  name: string;
  source: ClipSource;
}

/** A Unity material reduced to what a standard PBR material shows. */
export interface ModelMaterial {
  name: string;
  path: string | null;
  shader: string;
  approximated: boolean;
  unlit: boolean;
  color: [number, number, number, number];
  baseMap: string | null;
  normalMap: string | null;
  /** Names for the maps' contents at this version, for reusing loaded textures. */
  baseMapKey: string | null;
  normalMapKey: string | null;
  tiling: [number, number];
  offset: [number, number];
  metallic: number;
  smoothness: number;
  missing: boolean;
}

export type MeshRef =
  | { type: "asset"; path: string; name: string | null; id: string | null; scale: number }
  | { type: "builtin"; name: "Cube" | "Sphere" | "Capsule" | "Cylinder" | "Plane" | "Quad" }
  | { type: "missing" };

/** One GameObject of an assembled prefab, already in three.js's right-handed space. */
export interface ModelNode {
  name: string;
  parent: number | null;
  position: [number, number, number];
  rotation: [number, number, number, number];
  scale: [number, number, number];
  active: boolean;
  mesh: MeshRef | null;
  materials: number[];
  skinned: boolean;
  controller: string | null;
}

export interface ModelScene {
  nodes: ModelNode[];
  missing: string[];
  clips: SceneClip[];
}

export interface ModelInfo {
  kind: ModelKind;
  /** A name for the file's content at this version (blob id, or a work-tree file's size and time). */
  key: string | null;
  materials: ModelMaterial[];
  /** Material name inside the model → index in `materials`. */
  remaps: Record<string, number>;
  scale: number;
  scene: ModelScene | null;
}

export interface UnityYamlDiff {
  path: string;
  addedObjects: number;
  removedObjects: number;
  modifiedProperties: number;
  objects: UnityObjectChange[];
}

export interface UnityConflictRow {
  id: string;
  fileId: string;
  objectPath: string;
  component: string;
  property: string;
  base: string | null;
  yours: string | null;
  theirs: string | null;
  baseDisplay: string | null;
  yoursDisplay: string | null;
  theirsDisplay: string | null;
}

export interface UnityAutoRow {
  fileId: string;
  objectPath: string;
  component: string;
  property: string;
  side: string;
  value: string | null;
  display: string | null;
}

export interface UnityConflictTable {
  path: string;
  conflicts: UnityConflictRow[];
  autoMerged: UnityAutoRow[];
}

export interface UnityPropertyPick {
  fileId: string;
  property: string;
  value: string | null;
}

export interface UnityIntegrityIssue {
  path: string;
  kind: string;
  fileId: string | null;
  detail: string;
}

export interface UnityRepoSetupStatus {
  isUnity: boolean;
  hasGitignore: boolean;
  gitignoreMissing: string[];
  hasGitattributes: boolean;
  gitattributesMissing: string[];
  forceTextOk: boolean;
  serializationMode: number | null;
}

export interface MergeState {
  /** The branch being merged in, when gitgud started the merge. */
  theirs: string | null;
  /** Prefilled commit message. */
  message: string;
  /** In list order: Unity YAML, code, binary; then path. */
  conflicts: Conflict[];
  /** Per-property Unity Smart Merge verification items (v1.0). */
  unityVerify?: UnityMergeFile[];
}

export type Chunk =
  | { kind: "same"; lines: string[] }
  | { kind: "conflict"; yours: string[]; base: string[]; theirs: string[] };

export interface ConflictText {
  chunks: Chunk[];
  finalNewline: boolean;
}

export interface SideInfo {
  size: number;
  lfs: boolean;
  oid?: string | null;
  hydrated?: boolean;
}

/** `null` sides were deleted. */
export interface ConflictSides {
  yours: SideInfo | null;
  theirs: SideInfo | null;
}

export type MergeSide = "yours" | "theirs";

export interface StepConflict {
  /** 1-based. */
  step: number;
  shortId: string;
  subject: string;
  paths: string[];
}

export interface RebasePlan {
  /** Commits that would be replayed. */
  commits: number;
  /** Commits on the target the branch doesn't have. */
  behind: number;
  conflict: StepConflict | null;
}

export interface RebaseState {
  branch: string;
  onto: string;
  /** 1-based, of `total`. */
  step: number;
  total: number;
  /** Subject of the commit being applied. */
  subject: string;
  conflicts: Conflict[];
  /** Stopped by an Edit step: Continue amends the commit with what is staged. */
  editing: boolean;
}

export type RebaseStep = "continue" | "skip" | "abort";

export type PlanAction = "pick" | "reword" | "edit" | "squash" | "fixup" | "drop";

export interface PlanCommit {
  id: string;
  shortId: string;
  subject: string;
  message: string;
  author: string;
  time: number;
  /** Pick, or fixup / squash for `fixup!` / `squash!` commits. */
  action: PlanAction;
}

export interface HistoryPlan {
  branch: string;
  /** Parent of the oldest commit: the plan replays onto it unless moved. */
  base: string;
  baseShort: string;
  baseSubject: string;
  /** Newest first, fixup! / squash! already moved under their target. */
  commits: PlanCommit[];
  /** Other local branches on one of the commits (`--update-refs`). */
  dependents: string[];
  /** The upstream, when it has some of these commits. */
  pushed: string | null;
}

export interface PlanStep {
  id: string;
  action: PlanAction;
  message?: string | null;
}

export type StepOutcome =
  | { kind: "clean" }
  | { kind: "afterConflict"; step: number }
  | { kind: "conflict"; paths: string[]; tree: string }
  | { kind: "dropped" };

export interface StepPrediction {
  id: string;
  /** 1-based, oldest first; null for a drop. */
  step: number | null;
  outcome: StepOutcome;
}

export interface PlanPrediction {
  /** Newest first. */
  steps: StepPrediction[];
  before: number;
  after: number;
  /** Files the result differs in from HEAD; null when a conflict leaves it open. */
  changed: number | null;
}

export interface PlanPreview {
  prediction: PlanPrediction;
  /** Rows of the preview graph (`preview_rows`). */
  rows: number;
}

export type PullStrategy = "auto" | "ffOnly" | "merge" | "rebase";

export type PullOutcome =
  | { kind: "upToDate" }
  | { kind: "fastForwarded" }
  | { kind: "merged" }
  | { kind: "rebased" }
  | { kind: "conflicts" }
  | { kind: "diverged"; ahead: number; behind: number; upstream: string };

export interface PullResult {
  opened: Opened;
  outcome: PullOutcome;
}

export interface PushCommit {
  shortId: string;
  subject: string;
}

export interface PushInfo {
  branch: string;
  remote: string | null;
  remoteBranch: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  needsForce: boolean;
  commits: PushCommit[];
  remotes: string[];
  prUrl: string | null;
}

export type ResetMode = "soft" | "mixed" | "hard";

export type SequencerKind = "cherryPick" | "revert";

export interface SequencerState {
  kind: SequencerKind;
  commit: string;
  shortId: string;
  subject: string;
  message: string;
  conflicts: Conflict[];
}

export type SequencerStep = "continue" | "abort";

export interface StashEntry {
  index: number;
  id: string;
  shortId: string;
  message: string;
  branch: string;
  time: number;
  parentId: string;
}

export interface CommitOptions {
  amend?: boolean;
  signoff?: boolean;
  noVerify?: boolean;
}

export type HostKind = "gitHub" | "bitbucket" | "other" | "none";

export interface LibraryFolder {
  id: string;
  name: string;
  parentId: string | null;
  order: number;
}

export interface RepoSummary {
  path: string;
  missing: boolean;
  branch: string | null;
  detached: boolean;
  dirtyCount: number;
  untrackedCount: number;
  ahead: number;
  behind: number;
  fetchedAt: number | null;
  locksHeld: number;
  locksCheckedAt: number | null;
  lastActivity: number | null;
  host: HostKind;
  isUnity: boolean;
  checkedAt: number;
}

export interface LibraryRepo {
  id: string;
  name: string;
  path: string;
  folderId: string | null;
  pinned: boolean;
  order: number;
  lastOpened: number | null;
  cached: RepoSummary | null;
}

export interface OpenTabState {
  openPaths: string[];
  activePath: string | null;
}

export interface LibrarySnapshot {
  version: number;
  folders: LibraryFolder[];
  repos: LibraryRepo[];
  tabs: OpenTabState;
  undoAction: string | null;
  redoAction: string | null;
}

export interface AddRepoEntry {
  path: string;
  folderPath?: string | null;
  name?: string | null;
}

export interface ScannedRepo {
  name: string;
  path: string;
  alreadyInLibrary: boolean;
}

export interface ScanGroup {
  dir: string;
  suggestedFolder: string;
  repos: ScannedRepo[];
}

export interface ScanResult {
  root: string;
  totalFound: number;
  elapsedMs: number;
  groups: ScanGroup[];
}

export interface LfsProgress {
  state: string;
  filesDone: number;
  filesTotal: number;
  bytesDone: number;
  bytesTotal: number;
  currentFile: string;
}

export interface CloneProgress {
  phase: string;
  percent: number;
  done: number;
  total: number;
  lfs: LfsProgress | null;
}

export interface FetchManyProgress {
  done: number;
  total: number;
  path: string;
  error: string | null;
}

/** Staged: HEAD → index. Unstaged: index → work tree. */
export type Side = "staged" | "unstaged";

/** A hunk's `@@` numbers; Rust refuses the action if the diff moved since. */
export type HunkHeader = Pick<Hunk, "oldStart" | "oldLines" | "newStart" | "newLines">;

export const hunkHeader = (h: Hunk): HunkHeader => ({
  oldStart: h.oldStart,
  oldLines: h.oldLines,
  newStart: h.newStart,
  newLines: h.newLines,
});

export interface LineSelect {
  hunk: HunkHeader;
  lines: number[];
}

export interface IdentityConfig {
  userName: string | null;
  userEmail: string | null;
}

export interface ToolDiagnostics {
  gitPath: string;
  gitVersion: string;
  lfsVersion: string | null;
  gitSource?: "system" | "bundled";
  lfsOk?: boolean;
}

export type ProposalStatus = "pending" | "changesRequested" | "approved" | "rejected" | "withdrawn";

export interface RiskFlag {
  kind: string;
  path: string;
  detail: string;
}

export interface ProposalComment {
  id: string;
  author: string;
  text: string;
  path: string | null;
  time: number;
}

export interface ProposalSummary {
  id: string;
  commitId: string;
  shortCommit: string;
  baseId: string;
  message: string;
  why: string;
  agent: string;
  status: ProposalStatus;
  createdAt: number;
  updatedAt: number;
  files: string[];
  riskFlags: RiskFlag[];
  comments: ProposalComment[];
  headMoved: boolean;
}

export interface StackLevel {
  branch: string;
  base: string;
  commits: number;
  current: boolean;
  needsRestack: boolean;
  prUrl: string | null;
}

export interface BranchStack {
  root: string;
  levels: StackLevel[];
}

export interface LfsCacheStats {
  objectCount: number;
  totalBytes: number;
  prunableCount: number;
  prunableBytes: number;
}

/** What a Unity YAML-extension file holds, for picking its viewer. */
export interface UnityAssetInfo {
  /** Stored in Unity's binary format (Asset Serialization isn't Force Text). */
  binary: boolean;
  /** Unity class id of the first object: 114 MonoBehaviour, 43 Mesh, 21 Material… */
  classId: number | null;
  size: number;
}

/** One sprite of a texture, in Unity pixels (origin bottom-left). */
export interface Sprite {
  name: string;
  /** `[x, y, width, height]`; all 0 for a Single sprite (the whole texture). */
  rect: [number, number, number, number];
  pivot: [number, number];
  /** 9-slice borders: `[left, bottom, right, top]`. */
  border: [number, number, number, number];
}

export interface SpriteChange {
  name: string;
  kinds: ("added" | "removed" | "moved" | "resized" | "pivot" | "border")[];
  note: string;
}

/** A texture's slicing at two versions, from its `.meta`. */
export interface SpriteSheet {
  mode: "single" | "multiple" | "none";
  before: Sprite[] | null;
  after: Sprite[];
  changes: SpriteChange[];
}

/** An MP4 / MOV's video track, read from its boxes. */
export interface VideoInfo {
  codec: string;
  width: number;
  height: number;
  duration: number;
  fps: number | null;
  frames: number | null;
  /** Whole-file bits per second. */
  bitrate: number | null;
  audioCodec: string | null;
}

/** The tables of a CSV / TSV / JSON file or ScriptableObject, compared across two versions. */
export interface DataTables {
  /** "CSV", "JSON", "ScriptableObject · BoatDatabase"… */
  kind: string;
  /** The earlier version had the file, so rows carry changes. */
  compared: boolean;
  tables: TableDiff[];
}

export interface TableDiff {
  name: string;
  columns: string[];
  addedColumns: string[];
  removedColumns: string[];
  /** The column rows were matched by; null when matched by position. */
  key: string | null;
  rows: RowDiff[];
  rowCount: number;
  changed: number;
  changedCells: number;
  added: number;
  removed: number;
  truncated: boolean;
}

export interface RowDiff {
  state: "same" | "changed" | "added" | "removed";
  /** Per column `[old, new]`; null where that version has no such cell. */
  cells: [string | null, string | null][];
}

/** One version of a sound file, decoded in Rust. */
export interface AudioInfo {
  format: string;
  codec: string;
  /** The file's own rate. */
  sampleRate: number;
  channels: number;
  bitsPerSample: number | null;
  frames: number;
  duration: number;
  peakDbfs: number | null;
  loudnessLufs: number | null;
  /** Per channel, `[min, max]` pairs across the whole file. */
  peaks: [number, number][][];
}

/** Everything a `.mat` sets, for the material viewer. */
export interface MaterialView {
  material: ModelMaterial;
  properties: MaterialProperty[];
  keywords: string[];
  /** -1: the shader's own queue. */
  renderQueue: number | null;
}

export interface MaterialProperty {
  key: string;
  label: string;
  value: string;
  color: [number, number, number, number] | null;
  /** Left over from an earlier shader. */
  stale: boolean;
}

/** A font file's names, metrics and glyphs, for the font viewer. */
export interface FontInfo {
  /** "TrueType", "OpenType (CFF)", prefixed "WOFF · " / "WOFF2 · ", suffixed " · collection". */
  format: string;
  faces: number;
  family: string | null;
  style: string | null;
  version: string | null;
  weight: number;
  italic: boolean;
  unitsPerEm: number;
  ascender: number;
  descender: number;
  lineGap: number;
  xHeight: number | null;
  capHeight: number | null;
  glyphCount: number;
  axes: { tag: string; name: string | null; min: number; default: number; max: number }[];
  /** Every mapped Unicode code point, ascending. */
  codepoints: number[];
  /** Outline hash per code point (same order); 0 for an empty glyph. */
  outlines: number[];
}

/** What gitgud keeps for a repo to go faster (Settings › Caches). */
export interface CacheStats {
  guidIndexFile: string;
  guidIndexBytes: number;
  metaFiles: number;
  commitScans: number;
  metaBlobs: number;
  modelInfos: number;
  commitGraphPath: string;
  commitGraphBytes: number | null;
  lfsObjectsPath: string;
}

export interface AiProviderStatus {
  cliAvailable: string[];
  configuredEnvKey: boolean;
}

export interface TimingSample {
  op: string;
  elapsedMs: number;
  at: number;
}

export interface WorktreeInfo {
  path: string;
  branch: string | null;
  head: string;
  dirtyCount: number;
  isMain: boolean;
}

/** Every command the frontend invokes: its arguments and its Ok value. Errors arrive as strings. */
export interface Commands {
  git_info: { args: Record<string, never>; result: GitInfo };
  launch_path: { args: Record<string, never>; result: string | null };
  open_repo: { args: { path: string }; result: Opened };
  close_repo: { args: { id: number }; result: null };
  repo_status: { args: { id: number }; result: Status };
  graph_rows: { args: { id: number; start: number; count: number }; result: Row[] };
  graph_find: { args: { id: number; query: string }; result: number[] };
  commit_detail: { args: { id: number; commit: string }; result: CommitDetail };
  commit_file_diff: { args: { id: number; commit: string; path: string; ignoreWhitespace?: boolean }; result: FileDiff };
  worktree_file_diff: { args: { id: number; path: string; side: Side; ignoreWhitespace?: boolean }; result: FileDiff };
  stage_hunk: { args: { id: number; path: string; hunk: HunkHeader }; result: Opened };
  unstage_hunk: { args: { id: number; path: string; hunk: HunkHeader }; result: Opened };
  stage_lines: { args: { id: number; path: string; select: LineSelect }; result: Opened };
  unstage_lines: { args: { id: number; path: string; select: LineSelect }; result: Opened };
  discard: { args: { id: number; paths: string[] }; result: Opened };
  discard_hunk: { args: { id: number; path: string; hunk: HunkHeader }; result: Opened };
  discard_lines: { args: { id: number; path: string; select: LineSelect }; result: Opened };
  refresh_repo: { args: { id: number }; result: Opened };
  undo: { args: { id: number }; result: Reverted };
  redo: { args: { id: number }; result: Reverted };
  fetch: { args: { id: number }; result: Opened };
  op_log: { args: { id: number }; result: OpEntry[] };
  branches: { args: { id: number }; result: Branches };
  checkout: { args: { id: number; branch: string }; result: Opened };
  restore_to: { args: { id: number; entry: number }; result: Reverted };
  restore_files: { args: { id: number; entry: number }; result: Opened };
  cancel_fetch: { args: { id: number }; result: boolean };
  cancel_task: { args: { id: number }; result: boolean };
  pull: { args: { id: number; strategy: PullStrategy; remember?: boolean }; result: PullResult };
  push_info: { args: { id: number; remote?: string | null }; result: PushInfo };
  push: {
    args: {
      id: number;
      remote: string;
      branch: string;
      remoteBranch?: string | null;
      setUpstream: boolean;
      forceWithLease: boolean;
    };
    result: Opened;
  };
  delete_remote_branch: { args: { id: number; remote: string; branch: string }; result: Opened };
  create_branch: { args: { id: number; name: string; at?: string | null; checkout: boolean }; result: Opened };
  rename_branch: { args: { id: number; old: string; new: string }; result: Opened };
  delete_branch: { args: { id: number; name: string; force: boolean }; result: Opened };
  set_upstream: { args: { id: number; branch: string; upstream: string | null }; result: Opened };
  checkout_remote: { args: { id: number; remoteBranch: string; localName: string }; result: Opened };
  stage: { args: { id: number; paths: string[] }; result: Opened };
  unstage: { args: { id: number; paths: string[] }; result: Opened };
  commit: { args: { id: number; message: string; amend?: boolean; signoff?: boolean; noVerify?: boolean }; result: Opened };
  head_commit_message: { args: { id: number }; result: string | null };
  reset: { args: { id: number; target: string; mode: ResetMode }; result: Opened };
  predict_cherry_pick: { args: { id: number; commit: string }; result: string[] };
  predict_revert: { args: { id: number; commit: string }; result: string[] };
  cherry_pick: { args: { id: number; commit: string }; result: Opened };
  revert_commit: { args: { id: number; commit: string }; result: Opened };
  sequencer_state: { args: { id: number }; result: SequencerState | null };
  sequencer_step: { args: { id: number; step: SequencerStep }; result: Opened };
  stashes: { args: { id: number }; result: StashEntry[] };
  stash_detail: { args: { id: number; index: number }; result: CommitDetail };
  stash_file_diff: { args: { id: number; index: number; path: string; ignoreWhitespace?: boolean }; result: FileDiff };
  stash_save: { args: { id: number; message?: string | null; includeUntracked: boolean }; result: Opened };
  stash_apply: { args: { id: number; index: number }; result: Opened };
  stash_pop: { args: { id: number; index: number }; result: Opened };
  stash_drop: { args: { id: number; index: number }; result: Opened };
  config_get_identity: { args: { id?: number | null }; result: IdentityConfig };
  config_set_identity: { args: { id?: number | null; name: string; email: string }; result: null };
  tool_diagnostics: { args: Record<string, never>; result: ToolDiagnostics };
  unity_info: { args: { id: number }; result: UnityInfo };
  unity_configure_smart_merge: { args: { id: number; toolPath?: string | null }; result: UnityInfo };
  unity_meta_issues: { args: { id: number }; result: MetaIssue[] };
  unity_remerge_conflicts: { args: { id: number }; result: Opened };
  unity_confirm_verify: {
    args: { id: number; path: string; fileId?: string | null; property?: string | null; confirmed: boolean };
    result: Opened;
  };
  unity_check_open_assets: { args: { id: number; range: string }; result: boolean };
  lfs_info: { args: { id: number }; result: LfsInfo };
  lfs_locks: { args: { id: number }; result: LfsLock[] };
  lfs_lock: { args: { id: number; path: string }; result: null };
  lfs_unlock: { args: { id: number; path: string; force?: boolean }; result: null };
  lfs_locks_in_push: { args: { id: number; remote: string; remoteBranch: string; branch: string }; result: LfsLock[] };
  background_refresh: { args: { id: number; minAgeSecs?: number }; result: boolean };
  tags: { args: { id: number }; result: TagInfo[] };
  create_tag: { args: { id: number; name: string; target?: string | null; message?: string | null }; result: Opened };
  delete_tag: { args: { id: number; name: string }; result: Opened };
  push_tag: { args: { id: number; remote: string; name: string }; result: null };
  delete_remote_tag: { args: { id: number; remote: string; name: string }; result: null };
  file_history: { args: { id: number; path: string; limit?: number }; result: FileCommit[] };
  blame_file: { args: { id: number; path: string; rev?: string | null }; result: BlameLine[] };
  op_log_stats: { args: { id: number }; result: OpLogStats };
  op_log_prune: { args: { id: number }; result: OpLogStats };
  outside_reflog: { args: { id: number; limit?: number }; result: ReflogEntry[] };
  has_commit_graph: { args: { id: number }; result: boolean };
  cache_stats: { args: { id: number }; result: CacheStats };
  clear_unity_caches: { args: { id: number }; result: CacheStats };
  write_commit_graph: { args: { id: number }; result: null };
  worktree_unity_diff: { args: { id: number; path: string; side: Side }; result: UnityYamlDiff };
  commit_unity_diff: { args: { id: number; commit: string; path: string }; result: UnityYamlDiff };
  model_info: { args: { id: number; path: string; version: FileVersion }; result: ModelInfo };
  unity_asset_info: { args: { id: number; path: string; version: FileVersion }; result: UnityAssetInfo };
  open_version: { args: { id: number; path: string; version: FileVersion; label: string }; result: string };
  sprite_sheet: { args: { id: number; texture: string; before?: FileVersion | null; after: FileVersion }; result: SpriteSheet };
  video_info: { args: { id: number; path: string; version: FileVersion }; result: VideoInfo };
  data_tables: { args: { id: number; path: string; before?: FileVersion | null; after: FileVersion }; result: DataTables };
  audio_info: { args: { id: number; path: string; version: FileVersion }; result: AudioInfo };
  material_view: { args: { id: number; path: string; version: FileVersion }; result: MaterialView };
  material_users: { args: { id: number; path: string; version: FileVersion }; result: string[] };
  font_info: { args: { id: number; path: string; version: FileVersion; face?: number }; result: FontInfo };
  unity_conflict_table: { args: { id: number; path: string }; result: UnityConflictTable };
  resolve_unity_conflict: { args: { id: number; path: string; picks: UnityPropertyPick[] }; result: Opened };
  unity_post_merge_check: { args: { id: number; paths: string[] }; result: UnityIntegrityIssue[] };
  unity_repo_setup_status: { args: { id: number }; result: UnityRepoSetupStatus };
  unity_apply_repo_setup: {
    args: { id: number; writeGitignore: boolean; writeGitattributes: boolean };
    result: UnityRepoSetupStatus;
  };
  proposals_list: { args: { id: number }; result: ProposalSummary[] };
  proposal_file_diff: { args: { id: number; proposalId: string; path: string }; result: FileDiff };
  proposal_approve: {
    args: { id: number; proposalId: string; message?: string | null; selectedPaths: string[] };
    result: Opened;
  };
  proposal_request_changes: {
    args: { id: number; proposalId: string; comment: string; path?: string | null };
    result: ProposalSummary;
  };
  proposal_reject: { args: { id: number; proposalId: string; reason?: string | null }; result: Opened };
  branch_stacks: { args: { id: number }; result: BranchStack[] };
  restack: { args: { id: number; topBranch: string; onto: string }; result: Opened };
  pr_url_with_base: { args: { id: number; branch: string; base?: string | null }; result: string | null };
  lfs_cache_stats: { args: { id: number }; result: LfsCacheStats };
  lfs_prune: { args: { id: number }; result: LfsCacheStats };
  lfs_fetch_object: { args: { id: number; oid: string; size: number; path?: string | null }; result: boolean };
  ai_providers: { args: Record<string, never>; result: AiProviderStatus };
  ai_draft_commit_message: { args: { id: number; provider?: string | null }; result: string };
  agent_setup: { args: { id: number; target: string; withHooks?: boolean }; result: string };
  stale_origin_commits: { args: { id: number; upstream: string }; result: string[] };
  refresh_index: { args: { id: number }; result: Opened };
  recent_timings: { args: Record<string, never>; result: TimingSample[] };
  worktrees_list: { args: { id: number }; result: WorktreeInfo[] };
  propose_from_worktree: {
    args: { id: number; worktreePath: string; message: string; why?: string | null; agent?: string | null };
    result: ProposalSummary;
  };
  pr_url: { args: { id: number; branch: string }; result: string | null };
  open_external_url: { args: { url: string }; result: null };
  predict_merge: { args: { id: number; branch: string }; result: string[] };
  merge: { args: { id: number; branch: string }; result: Opened };
  merge_state: { args: { id: number }; result: MergeState | null };
  conflict_text: { args: { id: number; path: string }; result: ConflictText };
  conflict_sides: { args: { id: number; path: string }; result: ConflictSides };
  resolve_with: { args: { id: number; path: string; content: string; how: string }; result: Opened };
  resolve_side: { args: { id: number; path: string; side: MergeSide }; result: Opened };
  merge_abort: { args: { id: number }; result: Opened };
  merge_commit: { args: { id: number; message: string }; result: Opened };
  predict_rebase: { args: { id: number; onto: string }; result: RebasePlan };
  rebase: { args: { id: number; onto: string }; result: Opened };
  rebase_state: { args: { id: number }; result: RebaseState | null };
  rebase_step: { args: { id: number; step: RebaseStep }; result: Opened };
  history_plan: { args: { id: number; from: string }; result: HistoryPlan };
  plan_preview: { args: { id: number; onto: string; base: string; steps: PlanStep[] }; result: PlanPreview };
  preview_rows: { args: { id: number; start: number; count: number }; result: Row[] };
  predicted_file: { args: { id: number; tree: string; path: string }; result: string };
  run_plan: { args: { id: number; onto: string; base: string; steps: PlanStep[]; updateRefs: boolean }; result: Opened };
  reword_head: { args: { id: number; message: string }; result: Opened };
  quick_edit: { args: { id: number; commit: string; action: "fixup" | "drop" }; result: Opened };
  library_load: { args: Record<string, never>; result: LibrarySnapshot };
  library_summaries: { args: { paths: string[] }; result: RepoSummary[] };
  library_add_repos: { args: { entries: AddRepoEntry[] }; result: LibrarySnapshot };
  library_remove_repo: { args: { id: string }; result: LibrarySnapshot };
  library_locate_repo: { args: { id: string; newPath: string }; result: LibrarySnapshot };
  library_move_repo: { args: { id: string; folderId: string | null; order: number }; result: LibrarySnapshot };
  library_pin_repo: { args: { id: string; pinned: boolean }; result: LibrarySnapshot };
  library_create_folder: { args: { name: string; parentId: string | null }; result: LibrarySnapshot };
  library_rename_folder: { args: { id: string; name: string }; result: LibrarySnapshot };
  library_delete_folder: { args: { id: string }; result: LibrarySnapshot };
  library_move_folder: { args: { id: string; parentId: string | null; order: number }; result: LibrarySnapshot };
  library_save_tabs: { args: { tabs: OpenTabState }; result: LibrarySnapshot };
  library_undo: { args: Record<string, never>; result: LibrarySnapshot };
  library_redo: { args: Record<string, never>; result: LibrarySnapshot };
  scan_folder: { args: { root: string }; result: ScanResult };
  clone_repo: { args: { url: string; dest: string; folderPath: string | null; longPaths: boolean }; result: string };
  cancel_clone: { args: Record<string, never>; result: boolean };
  fetch_many: { args: { paths: string[] }; result: RepoSummary[] };
  "plugin:window|set_title": { args: { label: string; value: string }; result: null };
  "plugin:dialog|open": { args: { options: { directory: boolean; title: string } }; result: string | null };
}

function call<K extends keyof Commands>(cmd: K, args?: Commands[K]["args"]): Promise<Commands[K]["result"]> {
  return invoke<Commands[K]["result"]>(cmd, args);
}

export const gitInfo = () => call("git_info");
export const launchPath = () => call("launch_path");
export const openRepo = (path: string) => call("open_repo", { path });
export const closeRepo = (id: number) => call("close_repo", { id });
export const repoStatus = (id: number) => call("repo_status", { id });
export const graphRows = (id: number, start: number, count: number) => call("graph_rows", { id, start, count });
/** Rows matching `query` (subject, author, hash prefix, ref name), in graph order. */
export const graphFind = (id: number, query: string) => call("graph_find", { id, query });
export const commitDetail =(id: number, commit: string) => call("commit_detail", { id, commit });
export const commitFileDiff = (id: number, commit: string, path: string, ignoreWhitespace?: boolean) =>
  call("commit_file_diff", ignoreWhitespace ? { id, commit, path, ignoreWhitespace: true } : { id, commit, path });
export const worktreeFileDiff = (id: number, path: string, side: Side, ignoreWhitespace?: boolean) =>
  call("worktree_file_diff", ignoreWhitespace ? { id, path, side, ignoreWhitespace: true } : { id, path, side });
export const stageHunk = (id: number, path: string, hunk: HunkHeader) => call("stage_hunk", { id, path, hunk });
export const unstageHunk = (id: number, path: string, hunk: HunkHeader) => call("unstage_hunk", { id, path, hunk });
export const stageLines = (id: number, path: string, select: LineSelect) => call("stage_lines", { id, path, select });
export const unstageLines = (id: number, path: string, select: LineSelect) => call("unstage_lines", { id, path, select });
/** Undoable with Ctrl+Z: the work tree is snapshotted first. */
export const discard = (id: number, paths: string[]) => call("discard", { id, paths });
export const discardHunk = (id: number, path: string, hunk: HunkHeader) => call("discard_hunk", { id, path, hunk });
export const discardLines = (id: number, path: string, select: LineSelect) => call("discard_lines", { id, path, select });
export const refreshRepo = (id: number) => call("refresh_repo", { id });
export const undo = (id: number) => call("undo", { id });
export const redo = (id: number) => call("redo", { id });
export const fetchRemotes = (id: number) => call("fetch", { id });
/** Newest first. */
export const opLog = (id: number) => call("op_log", { id });
export const branches = (id: number) => call("branches", { id });
/** Undoable with Ctrl+Z: switches back and restores the files it rewrote. */
export const checkout = (id: number, branch: string) => call("checkout", { id, branch });
export const restoreTo = (id: number, entry: number) => call("restore_to", { id, entry });
export const restoreFiles = (id: number, entry: number) => call("restore_files", { id, entry });
export const cancelFetch = (id: number) => call("cancel_fetch", { id });
export const cancelTask = (id: number) => call("cancel_task", { id });
export const pull = (id: number, strategy: PullStrategy, remember = false) => call("pull", { id, strategy, remember });
export const pushInfo = (id: number, remote?: string | null) => call("push_info", { id, remote: remote ?? null });
export const push = (
  id: number,
  remote: string,
  branch: string,
  remoteBranch: string | null,
  setUpstream: boolean,
  forceWithLease: boolean,
) => call("push", { id, remote, branch, remoteBranch, setUpstream, forceWithLease });
export const deleteRemoteBranch = (id: number, remote: string, branch: string) =>
  call("delete_remote_branch", { id, remote, branch });
export const createBranch = (id: number, name: string, at: string | null, checkout: boolean) =>
  call("create_branch", { id, name, at, checkout });
export const renameBranch = (id: number, oldName: string, newName: string) =>
  call("rename_branch", { id, old: oldName, new: newName });
export const deleteBranch = (id: number, name: string, force: boolean) => call("delete_branch", { id, name, force });
export const setUpstream = (id: number, branch: string, upstream: string | null) =>
  call("set_upstream", { id, branch, upstream });
export const checkoutRemote = (id: number, remoteBranch: string, localName: string) =>
  call("checkout_remote", { id, remoteBranch, localName });
export const headCommitMessage = (id: number) => call("head_commit_message", { id });
export const resetRepo = (id: number, target: string, mode: ResetMode) => call("reset", { id, target, mode });
export const predictCherryPick = (id: number, commit: string) => call("predict_cherry_pick", { id, commit });
export const predictRevert = (id: number, commit: string) => call("predict_revert", { id, commit });
export const cherryPick = (id: number, commit: string) => call("cherry_pick", { id, commit });
export const revertCommit = (id: number, commit: string) => call("revert_commit", { id, commit });
export const sequencerState = (id: number) => call("sequencer_state", { id });
export const sequencerStep = (id: number, step: SequencerStep) => call("sequencer_step", { id, step });
export const stashes = (id: number) => call("stashes", { id });
export const stashDetail = (id: number, index: number) => call("stash_detail", { id, index });
export const stashFileDiff = (id: number, index: number, path: string, ignoreWhitespace?: boolean) =>
  call("stash_file_diff", ignoreWhitespace ? { id, index, path, ignoreWhitespace: true } : { id, index, path });
export const configGetIdentity = (id: number | null = null) => call("config_get_identity", { id });
export const configSetIdentity = (id: number | null, name: string, email: string) =>
  call("config_set_identity", { id, name, email });
export const toolDiagnostics = () => call("tool_diagnostics");
export const stashSave = (id: number, message: string | null, includeUntracked: boolean) =>
  call("stash_save", { id, message, includeUntracked });
export const stashApply = (id: number, index: number) => call("stash_apply", { id, index });
export const stashPop = (id: number, index: number) => call("stash_pop", { id, index });
export const stashDrop = (id: number, index: number) => call("stash_drop", { id, index });
export const prUrl = (id: number, branch: string) => call("pr_url", { id, branch });
export const openExternalUrl = (url: string) => call("open_external_url", { url });
/** Files merging `branch` would leave conflicted; touches nothing. */
export const predictMerge = (id: number, branch: string) => call("predict_merge", { id, branch });
/** Clean merges commit at once; undoable with Ctrl+Z either way. */
export const merge = (id: number, branch: string) => call("merge", { id, branch });
export const mergeState = (id: number) => call("merge_state", { id });
export const conflictText = (id: number, path: string) => call("conflict_text", { id, path });
export const conflictSides = (id: number, path: string) => call("conflict_sides", { id, path });
/** `how` names the pick in the op log, e.g. "use both". */
export const resolveWith = (id: number, path: string, content: string, how: string) =>
  call("resolve_with", { id, path, content, how });
export const resolveSide = (id: number, path: string, side: MergeSide) => call("resolve_side", { id, path, side });
export const mergeAbort = (id: number) => call("merge_abort", { id });
export const mergeCommit = (id: number, message: string) => call("merge_commit", { id, message });

export interface Progress {
  phase: string;
  percent: number;
  done: number;
  total: number;
}

export type TaskOp = "fetch" | "pull" | "push";

/** Calls `handler` with each progress line of a running fetch, pull or push. */
export const onTaskProgress = (handler: (id: number, op: TaskOp, progress: Progress) => void) =>
  listen<{ id: number; op: TaskOp; progress: Progress }>("task-progress", (e) =>
    handler(e.payload.id, e.payload.op, e.payload.progress),
  );

/** Calls `handler` with each progress line of a running fetch. Resolves to an unsubscribe. */
export const onFetchProgress = (handler: (id: number, progress: Progress) => void) =>
  listen<{ id: number; progress: Progress }>("fetch-progress", (e) => handler(e.payload.id, e.payload.progress));
export const stage = (id: number, paths: string[]) => call("stage", { id, paths });
export const unstage = (id: number, paths: string[]) => call("unstage", { id, paths });
export const commit = (id: number, message: string, options: CommitOptions = {}) =>
  call("commit", { id, message, ...options });

/** Calls `handler` with a repo id when its files or refs change outside gitgud. Resolves to an unsubscribe. */
export const onRepoChanged = (handler: (id: number) => void) => listen<number>("repo-changed", (e) => handler(e.payload));

/** What rebasing onto `onto` would do; touches nothing. */
export const predictRebase = (id: number, onto: string) => call("predict_rebase", { id, onto });
/** Stops on conflicts; undoable with Ctrl+Z at every step. */
export const rebase = (id: number, onto: string) => call("rebase", { id, onto });
export const rebaseState = (id: number) => call("rebase_state", { id });
export const rebaseStep = (id: number, step: RebaseStep) => call("rebase_step", { id, step });

/** The plan for editing the current branch from commit `from` up to HEAD. */
export const historyPlan = (id: number, from: string) => call("history_plan", { id, from });
/** Predicts the plan and lays out its graph preview, read with `previewRows`; touches nothing. */
export const planPreview = (id: number, onto: string, base: string, steps: PlanStep[]) =>
  call("plan_preview", { id, onto, base, steps });
export const previewRows = (id: number, start: number, count: number) => call("preview_rows", { id, start, count });
/** A predicted file, conflict markers included. */
export const predictedFile = (id: number, tree: string, path: string) => call("predicted_file", { id, tree, path });
/** Runs the plan as one undoable rebase; stops on a conflict or an Edit step. */
export const runPlan = (id: number, onto: string, base: string, steps: PlanStep[], updateRefs: boolean) =>
  call("run_plan", { id, onto, base, steps, updateRefs });
export const rewordHead = (id: number, message: string) => call("reword_head", { id, message });
/** Folds a commit into its parent or drops it, at once; undoable. */
export const quickEdit = (id: number, commit: string, action: "fixup" | "drop") => call("quick_edit", { id, commit, action });

/** Names the open repo in the native title bar. */
export const setWindowTitle = (value: string) => call("plugin:window|set_title", { label: "main", value });

/** Native folder picker; `null` when cancelled. */
export const pickFolder = (title = "Open repository") => open({ directory: true, title });

export const libraryLoad = () => call("library_load", {});
export const librarySummaries = (paths: string[]) => call("library_summaries", { paths });
export const libraryAddRepos = (entries: AddRepoEntry[]) => call("library_add_repos", { entries });
export const libraryRemoveRepo = (id: string) => call("library_remove_repo", { id });
export const libraryLocateRepo = (id: string, newPath: string) => call("library_locate_repo", { id, newPath });
export const libraryMoveRepo = (id: string, folderId: string | null, order: number) =>
  call("library_move_repo", { id, folderId, order });
export const libraryPinRepo = (id: string, pinned: boolean) => call("library_pin_repo", { id, pinned });
export const libraryCreateFolder = (name: string, parentId: string | null) =>
  call("library_create_folder", { name, parentId });
export const libraryRenameFolder = (id: string, name: string) => call("library_rename_folder", { id, name });
export const libraryDeleteFolder = (id: string) => call("library_delete_folder", { id });
export const libraryMoveFolder = (id: string, parentId: string | null, order: number) =>
  call("library_move_folder", { id, parentId, order });
export const librarySaveTabs = (tabs: OpenTabState) => call("library_save_tabs", { tabs });
export const libraryUndo = () => call("library_undo", {});
export const libraryRedo = () => call("library_redo", {});
export const scanFolder = (root: string) => call("scan_folder", { root });
export const cloneRepo = (url: string, dest: string, folderPath: string | null, longPaths: boolean) =>
  call("clone_repo", { url, dest, folderPath, longPaths });
export const cancelClone = () => call("cancel_clone", {});
export const fetchMany = (paths: string[]) => call("fetch_many", { paths });
export const unityInfo = (id: number) => call("unity_info", { id });
export const unityConfigureSmartMerge = (id: number, toolPath: string | null = null) =>
  call("unity_configure_smart_merge", { id, toolPath });
export const unityMetaIssues = (id: number) => call("unity_meta_issues", { id });
export const unityRemergeConflicts = (id: number) => call("unity_remerge_conflicts", { id });
export const unityConfirmVerify = (
  id: number,
  path: string,
  fileId: string | null,
  property: string | null,
  confirmed: boolean,
) => call("unity_confirm_verify", { id, path, fileId, property, confirmed });
export const unityCheckOpenAssets = (id: number, range: string) => call("unity_check_open_assets", { id, range });
export const lfsInfo = (id: number) => call("lfs_info", { id });
export const lfsLocks = (id: number) => call("lfs_locks", { id });
export const lfsLock = (id: number, path: string) => call("lfs_lock", { id, path });
export const lfsUnlock = (id: number, path: string, force = false) => call("lfs_unlock", { id, path, force });
export const lfsLocksInPush = (id: number, remote: string, remoteBranch: string, branch: string) =>
  call("lfs_locks_in_push", { id, remote, remoteBranch, branch });
export const backgroundRefresh = (id: number, minAgeSecs = 600) => call("background_refresh", { id, minAgeSecs });
export const tags = (id: number) => call("tags", { id });
export const createTag = (id: number, name: string, target: string | null = null, message: string | null = null) =>
  call("create_tag", { id, name, target, message });
export const deleteTag = (id: number, name: string) => call("delete_tag", { id, name });
export const pushTag = (id: number, remote: string, name: string) => call("push_tag", { id, remote, name });
export const deleteRemoteTag = (id: number, remote: string, name: string) =>
  call("delete_remote_tag", { id, remote, name });
export const fileHistory = (id: number, path: string, limit = 200) => call("file_history", { id, path, limit });
export const blameFile = (id: number, path: string, rev: string | null = null) =>
  call("blame_file", { id, path, rev });
export const opLogStats = (id: number) => call("op_log_stats", { id });
export const opLogPrune = (id: number) => call("op_log_prune", { id });
export const outsideReflog = (id: number, limit = 100) => call("outside_reflog", { id, limit });
export const hasCommitGraph = (id: number) => call("has_commit_graph", { id });
export const cacheStats = (id: number) => call("cache_stats", { id });
export const clearUnityCaches = (id: number) => call("clear_unity_caches", { id });
export const writeCommitGraph = (id: number) => call("write_commit_graph", { id });
export const worktreeUnityDiff = (id: number, path: string, side: Side) => call("worktree_unity_diff", { id, path, side });
export const commitUnityDiff = (id: number, commit: string, path: string) =>
  call("commit_unity_diff", { id, commit, path });
export const modelInfo = (id: number, path: string, version: FileVersion) => call("model_info", { id, path, version });
export const unityAssetInfo = (id: number, path: string, version: FileVersion) => call("unity_asset_info", { id, path, version });
/** Writes `path` at `version` to a temp copy, opens it in its default app, and returns where. */
export const openVersion = (id: number, path: string, version: FileVersion, label: string) =>
  call("open_version", { id, path, version, label });
export const spriteSheet = (id: number, texture: string, before: FileVersion | null, after: FileVersion) =>
  call("sprite_sheet", { id, texture, before, after });
export const videoInfo =(id: number, path: string, version: FileVersion) => call("video_info", { id, path, version });
export const dataTables =(id: number, path: string, before: FileVersion | null, after: FileVersion) =>
  call("data_tables", { id, path, before, after });
export const audioInfo =(id: number, path: string, version: FileVersion) => call("audio_info", { id, path, version });
export const materialView =(id: number, path: string, version: FileVersion) => call("material_view", { id, path, version });
export const materialUsers = (id: number, path: string, version: FileVersion) => call("material_users", { id, path, version });
export const fontInfo =(id: number, path: string, version: FileVersion) => call("font_info", { id, path, version });
export const unityConflictTable = (id: number, path: string) => call("unity_conflict_table", { id, path });
export const resolveUnityConflict = (id: number, path: string, picks: UnityPropertyPick[]) =>
  call("resolve_unity_conflict", { id, path, picks });
export const unityPostMergeCheck = (id: number, paths: string[]) => call("unity_post_merge_check", { id, paths });
export const unityRepoSetupStatus = (id: number) => call("unity_repo_setup_status", { id });
export const unityApplyRepoSetup = (id: number, writeGitignore: boolean, writeGitattributes: boolean) =>
  call("unity_apply_repo_setup", { id, writeGitignore, writeGitattributes });
export const proposalsList = (id: number) => call("proposals_list", { id });
export const proposalFileDiff = (id: number, proposalId: string, path: string) =>
  call("proposal_file_diff", { id, proposalId, path });
export const proposalApprove = (
  id: number,
  proposalId: string,
  selectedPaths: string[],
  message: string | null = null,
) => call("proposal_approve", { id, proposalId, message, selectedPaths });
export const proposalRequestChanges = (
  id: number,
  proposalId: string,
  comment: string,
  path: string | null = null,
) => call("proposal_request_changes", { id, proposalId, comment, path });
export const proposalReject = (id: number, proposalId: string, reason: string | null = null) =>
  call("proposal_reject", { id, proposalId, reason });
export const branchStacks = (id: number) => call("branch_stacks", { id });
export const restack = (id: number, topBranch: string, onto: string) => call("restack", { id, topBranch, onto });
export const prUrlWithBase = (id: number, branch: string, base: string | null = null) =>
  call("pr_url_with_base", { id, branch, base });
export const lfsCacheStats = (id: number) => call("lfs_cache_stats", { id });
export const lfsPrune = (id: number) => call("lfs_prune", { id });
export const lfsFetchObject = (id: number, oid: string, size: number, path: string | null = null) =>
  call("lfs_fetch_object", { id, oid, size, path });
export const aiProviders = () => call("ai_providers", {});
export const aiDraftCommitMessage = (id: number, provider: string | null = null) =>
  call("ai_draft_commit_message", { id, provider });
export const agentSetup = (id: number, target: string, withHooks = false) =>
  call("agent_setup", withHooks ? { id, target, withHooks: true } : { id, target });
export const staleOriginCommits = (id: number, upstream: string) => call("stale_origin_commits", { id, upstream });
export const refreshIndex = (id: number) => call("refresh_index", { id });
export const recentTimings = () => call("recent_timings", {});
export const worktreesList = (id: number) => call("worktrees_list", { id });
export const proposeFromWorktree = (
  id: number,
  worktreePath: string,
  message: string,
  why: string | null = null,
  agent: string | null = null,
) => call("propose_from_worktree", { id, worktreePath, message, why, agent });

export const onCloneProgress = (handler: (progress: CloneProgress) => void) =>
  listen<CloneProgress>("clone-progress", (e) => handler(e.payload));

export const onFetchManyProgress = (handler: (progress: FetchManyProgress) => void) =>
  listen<FetchManyProgress>("fetch-many-progress", (e) => handler(e.payload));
