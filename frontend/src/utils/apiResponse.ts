export type ApiResponseEnvelope<T = Record<string, unknown>> = {
  success?: boolean;
  data?: T;
  message?: string;
  error?: string;
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;

export const isApiSuccess = (payload: unknown): payload is ApiResponseEnvelope => {
  if (!isRecord(payload)) {
    return false;
  }

  return payload.success === true;
};

export const getApiData = <T = Record<string, unknown>>(payload: unknown): T | null => {
  if (!isApiSuccess(payload)) {
    return null;
  }

  if (!isRecord(payload.data)) {
    return null;
  }

  return payload.data as T;
};
