'use client';

import React, { useState } from 'react';
import { useRouter } from 'next/navigation';

export default function RegulatorLogin() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const router = useRouter();

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    try {
      // In production, this would call an authentication endpoint
      if (email && password) {
        // Simulate successful login
        const token = btoa(`${email}:${password}:${Date.now()}`);
        localStorage.setItem('regulator_token', token);
        localStorage.setItem('regulator_email', email);
        
        router.push('/regulator');
      } else {
        setError('Please enter both email and password');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="login-container">
      <div className="login-card">
        <h1>Regulator Portal</h1>
        <p className="subtitle">Secure Access to Audit Trails</p>

        <form onSubmit={handleLogin}>
          <div className="form-group">
            <label htmlFor="email">Email</label>
            <input
              id="email"
              type="email"
              placeholder="regulator@authority.gov"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              className="form-input"
            />
          </div>

          <div className="form-group">
            <label htmlFor="password">Password</label>
            <input
              id="password"
              type="password"
              placeholder="••••••••"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              className="form-input"
            />
          </div>

          {error && <div className="error-message">{error}</div>}

          <button
            type="submit"
            disabled={loading}
            className="btn-login"
          >
            {loading ? 'Signing in...' : 'Sign In'}
          </button>
        </form>

        <div className="info-section">
          <h3>Features</h3>
          <ul>
            <li>Query immutable audit trails</li>
            <li>Verify tamper-evidence chains</li>
            <li>Generate selective disclosure proofs</li>
            <li>Manage data sharing agreements</li>
            <li>Create compliance reports (ISA 3000, SOC2)</li>
            <li>Export audit data for analysis</li>
          </ul>
        </div>
      </div>

      <style jsx>{`
        .login-container {
          display: flex;
          align-items: center;
          justify-content: center;
          min-height: 100vh;
          background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }

        .login-card {
          background: white;
          padding: 40px;
          border-radius: 8px;
          box-shadow: 0 10px 40px rgba(0, 0, 0, 0.1);
          width: 100%;
          max-width: 400px;
        }

        h1 {
          margin: 0 0 8px;
          font-size: 28px;
          color: #333;
        }

        .subtitle {
          margin: 0 0 30px;
          color: #666;
          font-size: 14px;
        }

        .form-group {
          margin-bottom: 20px;
        }

        label {
          display: block;
          margin-bottom: 8px;
          font-weight: 500;
          color: #333;
        }

        .form-input {
          width: 100%;
          padding: 12px;
          border: 1px solid #ddd;
          border-radius: 4px;
          font-size: 14px;
          box-sizing: border-box;
          transition: border-color 0.3s;
        }

        .form-input:focus {
          outline: none;
          border-color: #667eea;
          box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
        }

        .error-message {
          background: #ffebee;
          color: #c62828;
          padding: 12px;
          border-radius: 4px;
          margin-bottom: 20px;
          font-size: 14px;
        }

        .btn-login {
          width: 100%;
          padding: 12px;
          background: #667eea;
          color: white;
          border: none;
          border-radius: 4px;
          font-size: 16px;
          font-weight: 600;
          cursor: pointer;
          transition: background 0.3s;
        }

        .btn-login:hover:not(:disabled) {
          background: #5568d3;
        }

        .btn-login:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .info-section {
          margin-top: 30px;
          padding-top: 30px;
          border-top: 1px solid #eee;
        }

        .info-section h3 {
          margin: 0 0 16px;
          font-size: 16px;
          color: #333;
        }

        .info-section ul {
          margin: 0;
          padding-left: 20px;
          list-style: none;
        }

        .info-section li {
          margin-bottom: 8px;
          color: #666;
          font-size: 14px;
          position: relative;
          padding-left: 20px;
        }

        .info-section li:before {
          content: '✓';
          position: absolute;
          left: 0;
          color: #4caf50;
          font-weight: bold;
        }
      `}</style>
    </div>
  );
}
