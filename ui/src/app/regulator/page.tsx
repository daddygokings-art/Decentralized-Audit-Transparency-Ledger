'use client';

import React, { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import Link from 'next/link';

interface AuditTrailEntry {
  eventIndex: number;
  eventHash: string;
  timestamp: number;
  eventType: string;
  submitter: string;
  sensitivity: string;
  controlEvent: boolean;
}

interface ComplianceReport {
  id: string;
  standard: string;
  auditSubject: string;
  status: string;
  eventsExamined: number;
  generatedAt: number;
}

export default function RegulatorPortal() {
  const [activeTab, setActiveTab] = useState<'audit-trail' | 'dsa' | 'compliance' | 'export'>('audit-trail');
  const [auditTrail, setAuditTrail] = useState<AuditTrailEntry[]>([]);
  const [reports, setReports] = useState<ComplianceReport[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filters, setFilters] = useState({
    startTime: '',
    endTime: '',
    eventTypes: [] as string[],
    onlyControlEvents: false,
  });

  const router = useRouter();

  useEffect(() => {
    // Check authentication
    const token = localStorage.getItem('regulator_token');
    if (!token) {
      router.push('/regulator/login');
    }
  }, [router]);

  const fetchAuditTrail = async () => {
    setLoading(true);
    setError(null);
    try {
      const token = localStorage.getItem('regulator_token');
      const params = new URLSearchParams();
      
      if (filters.startTime) params.append('startTime', filters.startTime);
      if (filters.endTime) params.append('endTime', filters.endTime);
      if (filters.eventTypes.length > 0) {
        filters.eventTypes.forEach(et => params.append('eventTypes', et));
      }
      if (filters.onlyControlEvents) params.append('onlyControlEvents', 'true');
      params.append('limit', '100');

      const response = await fetch(`/api/regulator/audit-trails?${params}`, {
        headers: {
          'Authorization': `Bearer ${token}`,
        },
      });

      if (!response.ok) throw new Error('Failed to fetch audit trail');
      const data = await response.json();
      setAuditTrail(data.entries || []);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch audit trail');
    } finally {
      setLoading(false);
    }
  };

  const fetchComplianceReports = async () => {
    setLoading(true);
    setError(null);
    try {
      const token = localStorage.getItem('regulator_token');
      const response = await fetch('/api/regulator/compliance-reports', {
        headers: {
          'Authorization': `Bearer ${token}`,
        },
      });

      if (!response.ok) throw new Error('Failed to fetch compliance reports');
      const data = await response.json();
      setReports(data || []);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch compliance reports');
    } finally {
      setLoading(false);
    }
  };

  const handleGenerateDisclosureProof = async (eventIndex: number) => {
    try {
      const token = localStorage.getItem('regulator_token');
      const response = await fetch('/api/regulator/selective-disclosure', {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          eventIndex,
          allowedFields: ['timestamp', 'eventType', 'submitter'],
          regulatorId: 'current_regulator',
        }),
      });

      if (!response.ok) throw new Error('Failed to generate proof');
      const proof = await response.json();
      
      // Display proof
      alert(`Proof generated:\n${JSON.stringify(proof, null, 2)}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to generate proof');
    }
  };

  const handleVerifyTamperEvidence = async (eventIndex: number) => {
    try {
      const token = localStorage.getItem('regulator_token');
      const response = await fetch(`/api/regulator/tamper-evidence/${eventIndex}`, {
        headers: {
          'Authorization': `Bearer ${token}`,
        },
      });

      if (!response.ok) throw new Error('Failed to verify tamper evidence');
      const verification = await response.json();
      
      alert(`Chain Valid: ${verification.chainValid}\nIntegrity Score: ${verification.verificationDetails.chainIntegrityScore}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to verify tamper evidence');
    }
  };

  const handleExportData = async () => {
    try {
      const token = localStorage.getItem('regulator_token');
      const response = await fetch('/api/regulator/export', {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          format: 'csv',
          startTime: filters.startTime ? Number(filters.startTime) : 0,
          endTime: filters.endTime ? Number(filters.endTime) : Date.now(),
        }),
      });

      if (!response.ok) throw new Error('Failed to export data');
      
      // Trigger download
      const blob = await response.blob();
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `audit_export_${Date.now()}.csv`;
      a.click();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to export data');
    }
  };

  return (
    <div className="regulator-portal">
      <header className="portal-header">
        <h1>Regulator Audit Portal</h1>
        <div className="header-actions">
          <button onClick={() => {
            localStorage.removeItem('regulator_token');
            router.push('/regulator/login');
          }} className="btn-secondary">Logout</button>
        </div>
      </header>

      <nav className="portal-nav">
        <button
          className={`tab ${activeTab === 'audit-trail' ? 'active' : ''}`}
          onClick={() => {
            setActiveTab('audit-trail');
            fetchAuditTrail();
          }}
        >
          Audit Trail
        </button>
        <button
          className={`tab ${activeTab === 'dsa' ? 'active' : ''}`}
          onClick={() => setActiveTab('dsa')}
        >
          Data Sharing Agreements
        </button>
        <button
          className={`tab ${activeTab === 'compliance' ? 'active' : ''}`}
          onClick={() => {
            setActiveTab('compliance');
            fetchComplianceReports();
          }}
        >
          Compliance Reports
        </button>
        <button
          className={`tab ${activeTab === 'export' ? 'active' : ''}`}
          onClick={() => setActiveTab('export')}
        >
          Export & Analytics
        </button>
      </nav>

      <main className="portal-content">
        {error && (
          <div className="error-banner">
            <span>{error}</span>
            <button onClick={() => setError(null)}>×</button>
          </div>
        )}

        {activeTab === 'audit-trail' && (
          <section className="audit-trail-section">
            <h2>Audit Trail Query</h2>
            
            <div className="filter-controls">
              <input
                type="datetime-local"
                placeholder="Start Time"
                value={filters.startTime}
                onChange={(e) => setFilters({...filters, startTime: e.target.value})}
                className="filter-input"
              />
              <input
                type="datetime-local"
                placeholder="End Time"
                value={filters.endTime}
                onChange={(e) => setFilters({...filters, endTime: e.target.value})}
                className="filter-input"
              />
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={filters.onlyControlEvents}
                  onChange={(e) => setFilters({...filters, onlyControlEvents: e.target.checked})}
                />
                Control Events Only
              </label>
              <button onClick={fetchAuditTrail} disabled={loading} className="btn-primary">
                {loading ? 'Loading...' : 'Query'}
              </button>
            </div>

            {auditTrail.length > 0 && (
              <div className="audit-table">
                <table>
                  <thead>
                    <tr>
                      <th>Index</th>
                      <th>Type</th>
                      <th>Submitter</th>
                      <th>Timestamp</th>
                      <th>Sensitivity</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {auditTrail.map((entry) => (
                      <tr key={entry.eventIndex}>
                        <td>{entry.eventIndex}</td>
                        <td>{entry.eventType}</td>
                        <td className="submitter">{entry.submitter.substring(0, 10)}...</td>
                        <td>{new Date(entry.timestamp).toLocaleString()}</td>
                        <td><span className={`sensitivity ${entry.sensitivity}`}>{entry.sensitivity}</span></td>
                        <td>
                          <button
                            onClick={() => handleGenerateDisclosureProof(entry.eventIndex)}
                            className="btn-small"
                          >
                            Proof
                          </button>
                          <button
                            onClick={() => handleVerifyTamperEvidence(entry.eventIndex)}
                            className="btn-small"
                          >
                            Verify
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>
        )}

        {activeTab === 'dsa' && (
          <section className="dsa-section">
            <h2>Data Sharing Agreements</h2>
            <p>Manage data sharing agreements with audit entities.</p>
            <Link href="/regulator/dsa" className="btn-primary">
              View & Manage DSAs
            </Link>
          </section>
        )}

        {activeTab === 'compliance' && (
          <section className="compliance-section">
            <h2>Compliance Reports</h2>
            
            {reports.length > 0 && (
              <div className="reports-grid">
                {reports.map((report) => (
                  <div key={report.id} className="report-card">
                    <h3>{report.standard}</h3>
                    <p><strong>Subject:</strong> {report.auditSubject}</p>
                    <p><strong>Status:</strong> <span className={`status ${report.status}`}>{report.status}</span></p>
                    <p><strong>Events Examined:</strong> {report.eventsExamined}</p>
                    <p><strong>Generated:</strong> {new Date(report.generatedAt).toLocaleDateString()}</p>
                    <Link href={`/regulator/compliance/${report.id}`} className="btn-small">
                      View Report
                    </Link>
                  </div>
                ))}
              </div>
            )}
          </section>
        )}

        {activeTab === 'export' && (
          <section className="export-section">
            <h2>Export & Analytics</h2>
            <button onClick={handleExportData} className="btn-primary">
              Export as CSV
            </button>
            <div className="export-info">
              <p>Download audit trail data for external analysis and compliance verification.</p>
            </div>
          </section>
        )}
      </main>

      <style jsx>{`
        .regulator-portal {
          display: flex;
          flex-direction: column;
          height: 100vh;
          background: #f5f5f5;
        }

        .portal-header {
          background: white;
          padding: 20px;
          border-bottom: 1px solid #ddd;
          display: flex;
          justify-content: space-between;
          align-items: center;
        }

        .portal-header h1 {
          margin: 0;
          font-size: 24px;
        }

        .header-actions button {
          padding: 8px 16px;
          background: #d32f2f;
          color: white;
          border: none;
          border-radius: 4px;
          cursor: pointer;
        }

        .portal-nav {
          background: white;
          border-bottom: 1px solid #ddd;
          display: flex;
          padding: 0 20px;
        }

        .tab {
          padding: 12px 16px;
          border: none;
          background: none;
          cursor: pointer;
          border-bottom: 3px solid transparent;
          font-weight: 500;
          transition: all 0.3s;
        }

        .tab.active {
          border-bottom-color: #1976d2;
          color: #1976d2;
        }

        .portal-content {
          flex: 1;
          overflow-y: auto;
          padding: 20px;
        }

        .filter-controls {
          display: flex;
          gap: 10px;
          margin-bottom: 20px;
          flex-wrap: wrap;
        }

        .filter-input {
          padding: 8px;
          border: 1px solid #ddd;
          border-radius: 4px;
        }

        .checkbox-label {
          display: flex;
          align-items: center;
          gap: 8px;
          cursor: pointer;
        }

        .btn-primary {
          padding: 10px 20px;
          background: #1976d2;
          color: white;
          border: none;
          border-radius: 4px;
          cursor: pointer;
          font-weight: 500;
        }

        .btn-primary:hover:not(:disabled) {
          background: #1565c0;
        }

        .btn-primary:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .btn-secondary {
          padding: 8px 16px;
          background: #757575;
          color: white;
          border: none;
          border-radius: 4px;
          cursor: pointer;
        }

        .btn-small {
          padding: 4px 8px;
          background: #2196f3;
          color: white;
          border: none;
          border-radius: 3px;
          cursor: pointer;
          font-size: 12px;
          margin-right: 4px;
        }

        .audit-table {
          background: white;
          border-radius: 4px;
          overflow-x: auto;
        }

        table {
          width: 100%;
          border-collapse: collapse;
        }

        th, td {
          padding: 12px;
          text-align: left;
          border-bottom: 1px solid #ddd;
        }

        th {
          background: #f5f5f5;
          font-weight: 600;
        }

        .submitter {
          font-family: monospace;
          font-size: 12px;
        }

        .sensitivity {
          padding: 4px 8px;
          border-radius: 3px;
          font-size: 12px;
          font-weight: 500;
        }

        .sensitivity.public { background: #e8f5e9; color: #2e7d32; }
        .sensitivity.internal { background: #fff3e0; color: #e65100; }
        .sensitivity.confidential { background: #fce4ec; color: #c2185b; }
        .sensitivity.restricted { background: #f3e5f5; color: #7b1fa2; }

        .error-banner {
          background: #ffebee;
          color: #c62828;
          padding: 12px 16px;
          border-radius: 4px;
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 20px;
        }

        .reports-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
          gap: 16px;
        }

        .report-card {
          background: white;
          padding: 16px;
          border-radius: 4px;
          border: 1px solid #ddd;
        }

        .report-card h3 {
          margin-top: 0;
        }

        .status {
          padding: 2px 6px;
          border-radius: 3px;
          font-size: 12px;
        }

        .status.draft { background: #e0e0e0; color: #424242; }
        .status.published { background: #e8f5e9; color: #2e7d32; }

        .export-info {
          background: white;
          padding: 16px;
          border-radius: 4px;
          margin-top: 20px;
        }
      `}</style>
    </div>
  );
}
