import axios, { AxiosInstance, AxiosError, InternalAxiosRequestConfig } from 'axios';
import { SessionManager } from '../auth/session';

export const createApiClient = (baseURL: string = '/'): AxiosInstance => {
  const client = axios.create({
    baseURL,
    headers: {
      'Content-Type': 'application/json',
    },
  });

  // Request interceptor to attach Bearer token
  client.interceptors.request.use(
    (config: InternalAxiosRequestConfig) => {
      const token = SessionManager.getToken();
      if (token) {
        config.headers.set('Authorization', `Bearer ${token}`);
      }
      return config;
    },
    (error) => Promise.reject(error)
  );

  // Response interceptor to handle 401 Unauthorized
  client.interceptors.response.use(
    (response) => response,
    (error: AxiosError) => {
      if (error.response && error.response.status === 401) {
        // Automatic 401 handling
        SessionManager.clearToken();
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
