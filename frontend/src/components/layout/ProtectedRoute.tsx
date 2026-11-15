import React, { useEffect } from 'react';
import { Navigate, Outlet, useNavigate } from 'react-router-dom';
import { SessionManager } from '../../auth/session';
import { api } from '../../api';

export const ProtectedRoute: React.FC = () => {
  const navigate = useNavigate();

  useEffect(() => {
    const handleUnauthorized = () => {
      SessionManager.clear();
      navigate('/login', { replace: true });
    };

    window.addEventListener('unauthorized', handleUnauthorized);

    if (SessionManager.isAuthenticated()) {
      api.getMe().catch(() => {
        SessionManager.clear();
        navigate('/login', { replace: true });
      });
    }

    return () => {
      window.removeEventListener('unauthorized', handleUnauthorized);
    };
  }, [navigate]);

  if (!SessionManager.isAuthenticated()) {
    return <Navigate to="/login" replace />;
  }

  return <Outlet />;
};
