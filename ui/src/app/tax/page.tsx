'use client';

import React, { useState, useEffect } from 'react';
import Link from 'next/link';

export default function TaxCompliancePortal() {
  const [activeTab, setActiveTab] = useState<'dashboard' | 'vat' | 'dst' | 'crypto' | 'transfer-pricing' | 'cbcr'>('dashboard');
  const [complianceStatus, setComplianceStatus] = useState<any>(null);
  const [reports, setReports] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    fetchComplianceStatus();
    fetchReports();
  }, []);

  const fetchComplianceStatus = async () => {
    try {
      const response = await fetch('/api/tax/compliance-status', {
        headers: { 'Authorization': `Bearer ${localStorage.getItem('tax_token')}` }
      });
      const data = await response.json();
      setComplianceStatus(data);
    } catch (error) {
      console.error('Failed to fetch compliance status', error);
    }
  };

  const fetchReports = async () => {
    try {
      const response = await fetch('/api/tax/reports', {
        headers: { 'Authorization': `Bearer ${localStorage.getItem('tax_token')}` }
      });
      const data = await response.json();
      setReports(data);
    } catch (error) {
      console.error('Failed to fetch reports', error);
    }
  };

  const handleVATCalculation = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    try {
      const formData = new FormData(e.target as HTMLFormElement);
      const response = await fetch('/api/tax/vat-determination', {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${localStorage.getItem('tax_token')}`,
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(Object.fromEntries(formData))
      });
      const result = await response.json();
      alert(`VAT Amount: €${(result.vatAmount / 100).toFixed(2)}\nTotal: €${(result.grossAmount / 100).toFixed(2)}`);
    } catch (error) {
      alert('VAT calculation failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="tax-portal">
      <header className="portal-header">
        <h1>Tax Compliance Portal</h1>
        <div className="header-info">
          <span className="entity-id">Entity: ENTITY_001</span>
          <span className="jurisdiction">Jurisdiction: EU</span>
        </div>
      </header>

      <nav className="portal-nav">
        <button className={`tab ${activeTab === 'dashboard' ? 'active' : ''}`} onClick={() => setActiveTab('dashboard')}>
          Dashboard
        </button>
        <button className={`tab ${activeTab === 'vat' ? 'active' : ''}`} onClick={() => setActiveTab('vat')}>
          VAT/GST
        </button>
        <button className={`tab ${activeTab === 'dst' ? 'active' : ''}`} onClick={() => setActiveTab('dst')}>
          Digital Services Tax
        </button>
        <button className={`tab ${activeTab === 'crypto' ? 'active' : ''}`} onClick={() => setActiveTab('crypto')}>
          Crypto Reporting
        </button>
        <button className={`tab ${activeTab === 'transfer-pricing' ? 'active' : ''}`} onClick={() => setActiveTab('transfer-pricing')}>
          Transfer Pricing
        </button>
        <button className={`tab ${activeTab === 'cbcr' ? 'active' : ''}`} onClick={() => setActiveTab('cbcr')}>
          Country-by-Country
        </button>
      </nav>

      <main className="portal-content">
        {/* DASHBOARD TAB */}
        {activeTab === 'dashboard' && (
          <section className="dashboard">
            <h2>Compliance Overview</h2>
            
            {complianceStatus && (
              <div className="compliance-grid">
                <div className="compliance-card">
                  <h3>VAT Filing</h3>
                  <p className="status compliant">✓ Compliant</p>
                  <p>Last Filed: {complianceStatus.complianceStatus.vatFiling.lastFiled}</p>
                  <p>Next Due: {complianceStatus.complianceStatus.vatFiling.nextDue}</p>
                </div>

                <div className="compliance-card">
                  <h3>Digital Services Tax</h3>
                  <p className="status applicable">Applicable</p>
                  <p>Last Calculated: {complianceStatus.complianceStatus.dstCalculation.lastCalculated}</p>
                  <p>Next Due: {complianceStatus.complianceStatus.dstCalculation.nextDue}</p>
                </div>

                <div className="compliance-card">
                  <h3>Crypto Reporting</h3>
                  <p className="status required">Required</p>
                  <p>Last Reported: {complianceStatus.complianceStatus.cryptoReporting.lastReported}</p>
                  <p>Next Due: {complianceStatus.complianceStatus.cryptoReporting.nextDue}</p>
                </div>

                <div className="compliance-card">
                  <h3>Transfer Pricing</h3>
                  <p className="status required">Required</p>
                  <p>Last Documented: {complianceStatus.complianceStatus.transferPricing.lastDocumented}</p>
                  <p>Next Review: {complianceStatus.complianceStatus.transferPricing.nextReview}</p>
                </div>
              </div>
            )}

            <div className="metrics">
              <div className="metric">
                <label>Compliance Risk Score</label>
                <div className="score-bar">
                  <div className="score-fill" style={{ width: `${complianceStatus?.riskScore || 0}%` }}></div>
                </div>
                <span>{complianceStatus?.riskScore || 0}/100</span>
              </div>
              <div className="metric">
                <label>Outstanding Liabilities</label>
                <span className="amount">€{complianceStatus?.outstandingLiabilities.toLocaleString() || 0}</span>
              </div>
            </div>
          </section>
        )}

        {/* VAT/GST TAB */}
        {activeTab === 'vat' && (
          <section className="vat-section">
            <h2>VAT/GST Determination</h2>
            
            <form onSubmit={handleVATCalculation} className="tax-form">
              <div className="form-group">
                <label>Supply Type</label>
                <select name="supplyType" required>
                  <option>Goods</option>
                  <option>Services</option>
                  <option>Digital Services</option>
                  <option>Intangibles</option>
                  <option>Construction</option>
                </select>
              </div>

              <div className="form-group">
                <label>Amount (€)</label>
                <input type="number" name="amount" required min="0" step="0.01" />
              </div>

              <div className="form-group">
                <label>Place of Supply</label>
                <select name="placeOfSupply" required>
                  <option>EU</option>
                  <option>UK</option>
                  <option>US</option>
                  <option>Canada</option>
                  <option>Australia</option>
                </select>
              </div>

              <div className="form-group">
                <label>Customer Jurisdiction</label>
                <select name="customerJurisdiction" required>
                  <option>EU</option>
                  <option>UK</option>
                  <option>US</option>
                  <option>Canada</option>
                  <option>Australia</option>
                </select>
              </div>

              <div className="form-group checkbox">
                <input type="checkbox" name="isB2B" id="isB2B" />
                <label htmlFor="isB2B">B2B Transaction</label>
              </div>

              <button type="submit" disabled={loading} className="btn-primary">
                {loading ? 'Calculating...' : 'Calculate VAT'}
              </button>
            </form>
          </section>
        )}

        {/* DIGITAL SERVICES TAX TAB */}
        {activeTab === 'dst' && (
          <section className="dst-section">
            <h2>Digital Services Tax</h2>
            <div className="info-box">
              <h3>DST Calculator</h3>
              <p>Enter your digital service details to determine DST applicability and rate.</p>
              
              <form className="tax-form">
                <div className="form-group">
                  <label>Service Category</label>
                  <select>
                    <option>Online Advertising</option>
                    <option>Online Marketplace</option>
                    <option>Social Media</option>
                    <option>Video Streaming</option>
                  </select>
                </div>

                <div className="form-group">
                  <label>Annual Revenue (€)</label>
                  <input type="number" min="0" step="1000" />
                </div>

                <div className="form-group">
                  <label>User Jurisdiction</label>
                  <select>
                    <option>EU (3%)</option>
                    <option>UK (2%)</option>
                    <option>India (4%)</option>
                    <option>Australia (3%)</option>
                  </select>
                </div>

                <button type="button" className="btn-primary">Calculate DST</button>
              </form>
            </div>
          </section>
        )}

        {/* CRYPTO REPORTING TAB */}
        {activeTab === 'crypto' && (
          <section className="crypto-section">
            <h2>Crypto Asset Reporting (CARF/DAC8)</h2>
            <div className="info-box">
              <h3>CARF Reporting Requirements</h3>
              <ul>
                <li>Report all crypto transactions exceeding threshold</li>
                <li>Calculate gains/losses using FIFO method</li>
                <li>Report year-end holdings</li>
                <li>Include counterparty information</li>
              </ul>
              
              <button className="btn-primary">Generate CARF Report</button>
            </div>
          </section>
        )}

        {/* TRANSFER PRICING TAB */}
        {activeTab === 'transfer-pricing' && (
          <section className="transfer-pricing-section">
            <h2>Transfer Pricing Analysis</h2>
            <div className="info-box">
              <h3>Arm's Length Price Validation</h3>
              <p>Analyze transfer pricing for related party transactions.</p>
              
              <form className="tax-form">
                <div className="form-group">
                  <label>Transfer Method</label>
                  <select>
                    <option>Comparable Uncontrolled Price (CUP)</option>
                    <option>Cost Plus</option>
                    <option>Resale Price</option>
                    <option>Profit Split</option>
                    <option>TNMM</option>
                  </select>
                </div>

                <div className="form-group">
                  <label>Transfer Price (€)</label>
                  <input type="number" min="0" step="1" />
                </div>

                <div className="form-group">
                  <label>Comparable Prices (€) - comma separated</label>
                  <input type="text" placeholder="100000,105000,95000" />
                </div>

                <button type="button" className="btn-primary">Analyze Price</button>
              </form>
            </div>
          </section>
        )}

        {/* COUNTRY-BY-COUNTRY REPORTING TAB */}
        {activeTab === 'cbcr' && (
          <section className="cbcr-section">
            <h2>Country-by-Country Reporting</h2>
            <div className="info-box">
              <h3>CbCR Filing</h3>
              <p>Generate country-by-country reporting for BEPS Action 13 compliance.</p>
              
              <div className="cbcr-preview">
                <table>
                  <thead>
                    <tr>
                      <th>Jurisdiction</th>
                      <th>Revenue</th>
                      <th>Profit</th>
                      <th>Tax Paid</th>
                      <th>Employees</th>
                      <th>Assets</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr>
                      <td>EU</td>
                      <td>€1,500,000</td>
                      <td>€300,000</td>
                      <td>€75,000</td>
                      <td>50</td>
                      <td>€2,000,000</td>
                    </tr>
                    <tr>
                      <td>UK</td>
                      <td>€800,000</td>
                      <td>€150,000</td>
                      <td>€30,000</td>
                      <td>25</td>
                      <td>€1,000,000</td>
                    </tr>
                  </tbody>
                </table>
              </div>

              <button className="btn-primary">Generate CbCR Report</button>
            </div>
          </section>
        )}
      </main>

      <style jsx>{`
        .tax-portal {
          display: flex;
          flex-direction: column;
          height: 100vh;
          background: #f5f5f5;
          font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', sans-serif;
        }

        .portal-header {
          background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
          color: white;
          padding: 24px;
          box-shadow: 0 2px 8px rgba(0,0,0,0.1);
        }

        .portal-header h1 {
          margin: 0 0 8px;
          font-size: 28px;
        }

        .header-info {
          display: flex;
          gap: 24px;
          font-size: 14px;
          opacity: 0.9;
        }

        .portal-nav {
          background: white;
          border-bottom: 1px solid #e0e0e0;
          display: flex;
          padding: 0 24px;
          gap: 8px;
          overflow-x: auto;
        }

        .tab {
          padding: 12px 16px;
          border: none;
          background: none;
          cursor: pointer;
          border-bottom: 3px solid transparent;
          font-weight: 500;
          transition: all 0.3s;
          white-space: nowrap;
        }

        .tab.active {
          border-bottom-color: #667eea;
          color: #667eea;
        }

        .portal-content {
          flex: 1;
          overflow-y: auto;
          padding: 24px;
        }

        .dashboard {
          background: white;
          border-radius: 8px;
          padding: 24px;
        }

        .compliance-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
          gap: 16px;
          margin-bottom: 32px;
        }

        .compliance-card {
          border: 1px solid #e0e0e0;
          border-radius: 8px;
          padding: 16px;
          background: #fafafa;
        }

        .compliance-card h3 {
          margin: 0 0 8px;
          font-size: 16px;
        }

        .status {
          font-weight: 600;
          margin: 8px 0;
          padding: 4px 8px;
          border-radius: 4px;
          display: inline-block;
          font-size: 12px;
        }

        .status.compliant {
          color: #2e7d32;
          background: #e8f5e9;
        }

        .status.applicable {
          color: #f57c00;
          background: #fff3e0;
        }

        .status.required {
          color: #c62828;
          background: #ffebee;
        }

        .metrics {
          display: grid;
          grid-template-columns: 1fr 1fr;
          gap: 24px;
          margin-top: 24px;
          padding-top: 24px;
          border-top: 1px solid #e0e0e0;
        }

        .metric label {
          display: block;
          font-weight: 500;
          margin-bottom: 8px;
          color: #424242;
        }

        .score-bar {
          width: 100%;
          height: 8px;
          background: #e0e0e0;
          border-radius: 4px;
          overflow: hidden;
          margin: 8px 0;
        }

        .score-fill {
          height: 100%;
          background: linear-gradient(90deg, #4caf50, #ffc107);
          transition: width 0.3s;
        }

        .amount {
          font-size: 20px;
          font-weight: 600;
          color: #667eea;
        }

        .tax-form {
          background: white;
          border-radius: 8px;
          padding: 24px;
          max-width: 500px;
        }

        .form-group {
          margin-bottom: 16px;
        }

        .form-group label {
          display: block;
          margin-bottom: 6px;
          font-weight: 500;
          color: #333;
        }

        .form-group input,
        .form-group select {
          width: 100%;
          padding: 10px;
          border: 1px solid #ddd;
          border-radius: 4px;
          font-size: 14px;
          box-sizing: border-box;
        }

        .form-group.checkbox {
          display: flex;
          align-items: center;
          gap: 8px;
        }

        .form-group.checkbox input {
          width: auto;
        }

        .form-group.checkbox label {
          margin: 0;
        }

        .btn-primary {
          background: #667eea;
          color: white;
          border: none;
          padding: 12px 24px;
          border-radius: 4px;
          font-size: 16px;
          font-weight: 600;
          cursor: pointer;
          transition: background 0.3s;
        }

        .btn-primary:hover:not(:disabled) {
          background: #5568d3;
        }

        .btn-primary:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .info-box {
          background: white;
          border-radius: 8px;
          padding: 24px;
        }

        .cbcr-preview table {
          width: 100%;
          border-collapse: collapse;
          margin: 16px 0;
          background: white;
        }

        .cbcr-preview th,
        .cbcr-preview td {
          padding: 12px;
          text-align: right;
          border-bottom: 1px solid #e0e0e0;
        }

        .cbcr-preview th {
          background: #f5f5f5;
          font-weight: 600;
          color: #333;
        }

        .cbcr-preview td:first-child,
        .cbcr-preview th:first-child {
          text-align: left;
        }
      `}</style>
    </div>
  );
}
