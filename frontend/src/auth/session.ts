export class SessionManager {
  private static readonly TOKEN_KEY = 'fusionpanel_auth_token';

  static getToken(): string | null {
    if (typeof localStorage !== 'undefined') {
      return localStorage.getItem(this.TOKEN_KEY);
    }
    return null; // For environments without localStorage (e.g., Node in tests without mock)
  }

  static setToken(token: string): void {
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(this.TOKEN_KEY, token);
    }
  }

  static clearToken(): void {
    if (typeof localStorage !== 'undefined') {
      localStorage.removeItem(this.TOKEN_KEY);
    }
  }
}
