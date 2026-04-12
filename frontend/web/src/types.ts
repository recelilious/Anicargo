export type ViewerSummary = {
  kind: "device" | "user";
  id: number | null;
  label: string;
  deviceId: string | null;
  isAdmin: boolean;
};

export type Policy = {
  subscriptionThreshold: number;
  replacementWindowHours: number;
  preferSameFansub: boolean;
  maxConcurrentDownloads: number;
  uploadLimitMb: number;
  downloadLimitMb: number;
};

export type BootstrapResponse = {
  deviceId: string;
  viewer: ViewerSummary;
  adminPath: string;
  policy: Policy;
};

export type Weekday = {
  id: number;
  cn: string;
  en: string;
  ja: string;
};

export type SubjectCard = {
  bangumiSubjectId: number;
  title: string;
  titleCn: string;
  summary: string;
  releaseStatus: "airing" | "completed" | "upcoming";
  airDate: string | null;
  broadcastTime: string | null;
  airWeekday: number | null;
  imagePortrait: string | null;
  imageBanner: string | null;
  tags: string[];
  totalEpisodes: number | null;
  ratingScore: number | null;
  catalogLabel: string | null;
};

export type CalendarDay = {
  weekday: Weekday;
  items: SubjectCard[];
};

export type CalendarResponse = {
  days: CalendarDay[];
};

export type CatalogManifestResponse = {
  previewAvailable: boolean;
  specialAvailable: boolean;
};

export type CatalogSection = {
  key: string;
  title: string;
  items: SubjectCard[];
};

export type CatalogPageResponse = {
  kind: string;
  title: string;
  sections: CatalogSection[];
};

export type SearchResponse = {
  items: SubjectCard[];
  facets: {
    years: number[];
    tags: string[];
  };
  total: number;
  page: number;
  pageSize: number;
  hasNextPage: boolean;
};

export type InfoboxItem = {
  key: string;
  value: string;
};

export type Episode = {
  bangumiEpisodeId: number;
  sort: number;
  episodeNumber: number | null;
  title: string;
  titleCn: string;
  airdate: string | null;
  durationSeconds: number | null;
  isAvailable: boolean;
  availabilityNote: string | null;
};

export type SubscriptionState = {
  isSubscribed: boolean;
  subscriptionCount: number;
  threshold: number;
  source: ViewerSummary;
};

export type SubjectDetailResponse = {
  subject: {
    bangumiSubjectId: number;
    title: string;
    titleCn: string;
    summary: string;
    releaseStatus: "airing" | "completed" | "upcoming";
    airDate: string | null;
    broadcastTime: string | null;
    airWeekday: number | null;
    totalEpisodes: number | null;
    imagePortrait: string | null;
    imageBanner: string | null;
    tags: string[];
    infobox: InfoboxItem[];
    ratingScore: number | null;
    openingThemes: string[];
    endingThemes: string[];
    relatedSubjects: SubjectCard[];
  };
  episodes: Episode[];
  subscription: SubscriptionState;
  downloadStatus: SubjectDownloadStatus | null;
};

export type SubjectDownloadStatus = {
  bangumiSubjectId: number;
  releaseStatus: string;
  demandState: string;
  subscriptionCount: number;
  thresholdSnapshot: number;
  lastQueuedJobId: number | null;
  jobLifecycle: string | null;
  searchStatus: string | null;
  selectedCandidateId: number | null;
  selectedTitle: string | null;
  executionId: number | null;
  executionState: string | null;
  sourceTitle: string | null;
  sourceFansubName: string | null;
  downloadedBytes: number;
  totalBytes: number;
  downloadRateBytes: number;
  uploadRateBytes: number;
  peerCount: number;
  readyMediaCount: number;
  latestReadyEpisode: number | null;
  lastReadyAt: string | null;
  lastEvaluatedAt: string;
};

export type EpisodePlaybackResponse = {
  bangumiSubjectId: number;
  bangumiEpisodeId: number;
  episodeNumber: number | null;
  availabilityState: "ready" | "downloading" | "missing" | "unmapped";
  note: string;
  media: {
    mediaInventoryId: number;
    fileName: string;
    fileExt: string;
    sizeBytes: number;
    sourceTitle: string;
    sourceFansubName: string | null;
    updatedAt: string;
    streamUrl: string;
    subtitleTracks: Array<{
      id: string;
      label: string;
      language: string | null;
      kind: string;
      url: string;
    }>;
  } | null;
};

export type AuthResponse = {
  token: string;
  viewer: ViewerSummary;
};

export type AdminDashboardResponse = {
  adminUsername: string;
  policy: Policy;
  fansubRules: Array<{
    id: number;
    fansubName: string;
    localePreference: string;
    priority: number;
    isBlacklist: boolean;
  }>;
  counts: {
    devices: number;
    users: number;
    subscriptions: number;
    fansubRules: number;
  };
};

export type AdminRuntimeResponse = {
  serverAddress: string;
  uptimeSeconds: number;
  uptimeLabel: string;
  logDir: string;
  downloadEngine: string;
  http: {
    activeRequests: number;
    totalRequests: number;
    failedRequests: number;
    incomingBytes: number;
    outgoingBytes: number;
    lastRoute: string;
    lastStatus: number;
    lastLatencyMs: number;
  };
  runtime: {
    devices: number;
    users: number;
    activeSessions: number;
    subscriptions: number;
    openDownloadJobs: number;
    jobsWithSelection: number;
    runningSearches: number;
    resourceCandidates: number;
    activeExecutions: number;
    downloadedBytes: number;
    uploadedBytes: number;
    downloadRateBytes: number;
    uploadRateBytes: number;
    peerCount: number;
  };
};

export type DownloadJob = {
  id: number;
  bangumiSubjectId: number;
  triggerKind: string;
  requestedBy: string;
  releaseStatus: string;
  seasonMode: string;
  lifecycle: string;
  subscriptionCount: number;
  thresholdSnapshot: number;
  engineName: string;
  engineJobRef: string | null;
  notes: string | null;
  selectedCandidateId: number | null;
  selectionUpdatedAt: string | null;
  lastSearchRunId: number | null;
  searchStatus: string;
  createdAt: string;
  updatedAt: string;
};

export type ResourceCandidate = {
  id: number;
  downloadJobId: number;
  searchRunId: number;
  bangumiSubjectId: number;
  slotKey: string;
  episodeIndex: number | null;
  episodeEndIndex: number | null;
  isCollection: boolean;
  provider: string;
  providerResourceId: string;
  title: string;
  href: string;
  magnet: string;
  releaseType: string;
  sizeBytes: number;
  fansubName: string | null;
  publisherName: string;
  sourceCreatedAt: string;
  sourceFetchedAt: string;
  resolution: string | null;
  localeHint: string | null;
  isRaw: boolean;
  score: number;
  rejectedReason: string | null;
  discoveredAt: string;
};

export type DownloadExecution = {
  id: number;
  downloadJobId: number;
  resourceCandidateId: number;
  bangumiSubjectId: number;
  slotKey: string;
  episodeIndex: number | null;
  episodeEndIndex: number | null;
  isCollection: boolean;
  engineName: string;
  engineExecutionRef: string | null;
  executionRole: string;
  state: string;
  targetPath: string;
  sourceTitle: string;
  sourceMagnet: string;
  sourceSizeBytes: number;
  sourceFansubName: string | null;
  downloadedBytes: number;
  uploadedBytes: number;
  downloadRateBytes: number;
  uploadRateBytes: number;
  peerCount: number;
  notes: string | null;
  createdAt: string;
  updatedAt: string;
  startedAt: string | null;
  completedAt: string | null;
  replacedAt: string | null;
  failedAt: string | null;
  lastIndexedAt: string | null;
};

export type DownloadExecutionEvent = {
  id: number;
  downloadExecutionId: number;
  level: string;
  eventKind: string;
  message: string;
  downloadedBytes: number | null;
  uploadedBytes: number | null;
  downloadRateBytes: number | null;
  uploadRateBytes: number | null;
  peerCount: number | null;
  createdAt: string;
};

export type ResourceLibraryItem = {
  id: number;
  bangumiSubjectId: number;
  downloadJobId: number;
  downloadExecutionId: number;
  resourceCandidateId: number;
  slotKey: string;
  sourceTitle: string;
  sourceFansubName: string | null;
  executionState: string;
  relativePath: string;
  absolutePath: string;
  fileName: string;
  fileExt: string;
  sizeBytes: number;
  episodeIndex: number | null;
  episodeEndIndex: number | null;
  isCollection: boolean;
  status: string;
  updatedAt: string;
};

export type ResourceLibraryResponse = {
  items: ResourceLibraryItem[];
  total: number;
  totalSizeBytes: number;
  page: number;
  pageSize: number;
  hasNextPage: boolean;
};

export type SubjectCollectionResponse = {
  items: SubjectCard[];
  total: number;
  page: number;
  pageSize: number;
  hasNextPage: boolean;
};

export type ActiveDownload = {
  bangumiSubjectId: number;
  title: string;
  titleCn: string;
  imagePortrait: string | null;
  releaseStatus: "airing" | "completed" | "upcoming";
  slotKey: string;
  episodeIndex: number | null;
  episodeEndIndex: number | null;
  isCollection: boolean;
  state: string;
  sourceTitle: string;
  sourceFansubName: string | null;
  downloadedBytes: number;
  totalBytes: number;
  downloadRateBytes: number;
  uploadRateBytes: number;
  peerCount: number;
  updatedAt: string;
};

export type ActiveDownloadsResponse = {
  items: ActiveDownload[];
};

export type PlaybackHistoryItem = {
  bangumiSubjectId: number;
  bangumiEpisodeId: number;
  episodeNumber: number | null;
  subjectTitle: string;
  subjectTitleCn: string;
  episodeTitle: string;
  episodeTitleCn: string;
  imagePortrait: string | null;
  fileName: string | null;
  sourceFansubName: string | null;
  lastPlayedAt: string;
  playCount: number;
};

export type PlaybackHistoryResponse = {
  items: PlaybackHistoryItem[];
  total: number;
  page: number;
  pageSize: number;
  hasNextPage: boolean;
};
