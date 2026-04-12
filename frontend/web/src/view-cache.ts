import { fetchEpisodePlayback, fetchSubjectDetail } from "./api";
import type { EpisodePlaybackResponse, SubjectCard, SubjectDetailResponse } from "./types";

const subjectPreviewCache = new Map<number, SubjectCard>();
const subjectDetailCache = new Map<string, SubjectDetailResponse>();
const subjectDetailRequestCache = new Map<string, Promise<SubjectDetailResponse>>();
const playbackCache = new Map<string, EpisodePlaybackResponse>();
const playbackRequestCache = new Map<string, Promise<EpisodePlaybackResponse>>();

function createViewerKey(deviceId: string, userToken: string | null) {
  return `${deviceId}:${userToken ?? "guest"}`;
}

function createSubjectDetailKey(subjectId: number, deviceId: string, userToken: string | null) {
  return `${createViewerKey(deviceId, userToken)}:subject:${subjectId}`;
}

function createPlaybackKey(
  subjectId: number,
  episodeId: number,
  deviceId: string,
  userToken: string | null,
) {
  return `${createViewerKey(deviceId, userToken)}:playback:${subjectId}:${episodeId}`;
}

export function subjectCardFromDetail(subject: SubjectDetailResponse["subject"]): SubjectCard {
  return {
    bangumiSubjectId: subject.bangumiSubjectId,
    title: subject.title,
    titleCn: subject.titleCn,
    summary: subject.summary,
    releaseStatus: subject.releaseStatus,
    airDate: subject.airDate,
    broadcastTime: subject.broadcastTime,
    airWeekday: subject.airWeekday,
    imagePortrait: subject.imagePortrait,
    imageBanner: subject.imageBanner,
    tags: subject.tags,
    totalEpisodes: subject.totalEpisodes,
    ratingScore: subject.ratingScore,
    catalogLabel: null,
  };
}

export function subjectDetailPreviewFromCard(subject: SubjectCard): SubjectDetailResponse["subject"] {
  return {
    bangumiSubjectId: subject.bangumiSubjectId,
    title: subject.title,
    titleCn: subject.titleCn,
    summary: subject.summary,
    releaseStatus: subject.releaseStatus,
    airDate: subject.airDate,
    broadcastTime: subject.broadcastTime,
    airWeekday: subject.airWeekday,
    totalEpisodes: subject.totalEpisodes,
    imagePortrait: subject.imagePortrait,
    imageBanner: subject.imageBanner,
    tags: subject.tags,
    infobox: [],
    ratingScore: subject.ratingScore,
    openingThemes: [],
    endingThemes: [],
    relatedSubjects: [],
  };
}

export function primeSubjectPreview(subject: SubjectCard) {
  if (subject.bangumiSubjectId > 0) {
    subjectPreviewCache.set(subject.bangumiSubjectId, subject);
  }
}

export function getCachedSubjectPreview(subjectId: number) {
  return subjectPreviewCache.get(subjectId) ?? null;
}

export function primeSubjectDetail(
  detail: SubjectDetailResponse,
  deviceId: string,
  userToken: string | null,
) {
  subjectDetailCache.set(
    createSubjectDetailKey(detail.subject.bangumiSubjectId, deviceId, userToken),
    detail,
  );
  primeSubjectPreview(subjectCardFromDetail(detail.subject));
}

export function getCachedSubjectDetail(
  subjectId: number,
  deviceId: string,
  userToken: string | null,
) {
  return subjectDetailCache.get(createSubjectDetailKey(subjectId, deviceId, userToken)) ?? null;
}

export async function fetchSubjectDetailCached(
  subjectId: number,
  deviceId: string,
  userToken: string | null,
) {
  const cacheKey = createSubjectDetailKey(subjectId, deviceId, userToken);
  const cached = subjectDetailCache.get(cacheKey);
  if (cached) {
    return cached;
  }

  let request = subjectDetailRequestCache.get(cacheKey);
  if (!request) {
    request = fetchSubjectDetail(subjectId, deviceId, userToken)
      .then((response) => {
        primeSubjectDetail(response, deviceId, userToken);
        subjectDetailRequestCache.delete(cacheKey);
        return response;
      })
      .catch((error) => {
        subjectDetailRequestCache.delete(cacheKey);
        throw error;
      });

    subjectDetailRequestCache.set(cacheKey, request);
  }

  return request;
}

export async function revalidateSubjectDetail(
  subjectId: number,
  deviceId: string,
  userToken: string | null,
) {
  const response = await fetchSubjectDetail(subjectId, deviceId, userToken);
  primeSubjectDetail(response, deviceId, userToken);
  return response;
}

export function primeEpisodePlayback(
  playback: EpisodePlaybackResponse,
  deviceId: string,
  userToken: string | null,
) {
  playbackCache.set(
    createPlaybackKey(
      playback.bangumiSubjectId,
      playback.bangumiEpisodeId,
      deviceId,
      userToken,
    ),
    playback,
  );
}

export function getCachedEpisodePlayback(
  subjectId: number,
  episodeId: number,
  deviceId: string,
  userToken: string | null,
) {
  return playbackCache.get(createPlaybackKey(subjectId, episodeId, deviceId, userToken)) ?? null;
}

export async function fetchEpisodePlaybackCached(
  subjectId: number,
  episodeId: number,
  deviceId: string,
  userToken: string | null,
) {
  const cacheKey = createPlaybackKey(subjectId, episodeId, deviceId, userToken);
  const cached = playbackCache.get(cacheKey);
  if (cached) {
    return cached;
  }

  let request = playbackRequestCache.get(cacheKey);
  if (!request) {
    request = fetchEpisodePlayback(subjectId, episodeId, deviceId, userToken)
      .then((response) => {
        primeEpisodePlayback(response, deviceId, userToken);
        playbackRequestCache.delete(cacheKey);
        return response;
      })
      .catch((error) => {
        playbackRequestCache.delete(cacheKey);
        throw error;
      });

    playbackRequestCache.set(cacheKey, request);
  }

  return request;
}

export async function revalidateEpisodePlayback(
  subjectId: number,
  episodeId: number,
  deviceId: string,
  userToken: string | null,
) {
  const response = await fetchEpisodePlayback(subjectId, episodeId, deviceId, userToken);
  primeEpisodePlayback(response, deviceId, userToken);
  return response;
}
