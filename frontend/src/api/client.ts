import axios, { AxiosInstance, AxiosError } from 'axios';
import { SessionManager } from '../auth/session';

export const createApiClient = (baseURL: string = '/'): AxiosInstance => {
  const client = axios.create({
    baseURL,
    withCredentials: true,
    headers: {
      'Content-Type': 'application/json',
    },
  });

  // Response interceptor to handle 401 Unauthorized
  client.interceptors.response.use(
    (response) => response,
    (error: AxiosError) => {
      if (error.response && error.response.status === 401) {
        // Automatic 401 handling
        SessionManager.clear();
        // Here you might want to dispatch an event or redirect to login
        if (typeof window !== 'undefined') {
          window.dispatchEvent(new Event('unauthorized'));
        }
      }
      return Promise.reject(error);
    }
  );

  return client;
};

export const apiClient = createApiClient();
