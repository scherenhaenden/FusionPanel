export class SessionManager {
  // This marker is only a UI hint. The credential itself stays in the
  // backend's HttpOnly SESSION_ID cookie and is never exposed to JavaScript.
  private static readonly SESSION_KEY = 'fusionpanel_session_authenticated';

  static isAuthenticated(): boolean {
    if (typeof localStorage !== 'undefined') {
      return localStorage.getItem(this.SESSION_KEY) === '1';
    }
    return false;
  }

  static markAuthenticated(): void {
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(this.SESSION_KEY, '1');
    }
  }

  static clear(): void {
    if (typeof localStorage !== 'undefined') {
      localStorage.removeItem(this.SESSION_KEY);
    }
  }
}
