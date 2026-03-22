export type ViewerSummary = {
  kind: "device" | "user";
  id: number | null;
  label: string;
  deviceId: string | null;
};

export type Policy = {
  subscriptionThreshold: number;
  replacementWindowHours: number;
  preferSameFansub: boolean;
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
};

export type CalendarDay = {
  weekday: Weekday;
  items: SubjectCard[];
};

export type CalendarResponse = {
  days: CalendarDay[];
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
    airDate: string | null;
    broadcastTime: string | null;
    airWeekday: number | null;
    totalEpisodes: number | null;
    imagePortrait: string | null;
    imageBanner: string | null;
    tags: string[];
    infobox: InfoboxItem[];
    ratingScore: number | null;
  };
  episodes: Episode[];
  subscription: SubscriptionState;
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
  } | null;
};

export type AuthResponse = {
  token: string;
  viewer: ViewerSummary;
};

export type AdminAuthResponse = {
  token: string;
  adminUsername: string;
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
