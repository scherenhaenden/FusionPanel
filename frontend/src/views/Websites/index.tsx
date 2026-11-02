import React, { useState } from 'react';
import { SiteList, Site } from './SiteList';
import { SiteDetail } from './SiteDetail';
import { CreateSiteModal, CreateSiteFormData } from './CreateSiteModal';

export const WebsitesView: React.FC = () => {
  const [sites, setSites] = useState<Site[]>([]);
  const [selectedSite, setSelectedSite] = useState<Site | null>(null);
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);

  const handleCreateSite = (data: CreateSiteFormData) => {
    const newSite: Site = {
      id: Math.random().toString(36).substr(2, 9),
      domainName: data.domainName,
      isActive: true, // newly created sites are active by default?
      isSslEnabled: false, // SSL needs to be provisioned separately usually
      phpVersion: data.isPhpEnabled ? '8.1' : undefined, // hardcoded default for now, could be dynamic
    };
    setSites([...sites, newSite]);
    setIsCreateModalOpen(false);
  };

  return (
    <div className="websites-view">
      <div className="websites-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
        <h1>Websites & Virtual Hosts</h1>
        {!selectedSite && (
          <button onClick={() => setIsCreateModalOpen(true)}>Create Site</button>
        )}
      </div>

      {selectedSite ? (
        <SiteDetail site={selectedSite} onClose={() => setSelectedSite(null)} />
      ) : (
        <SiteList sites={sites} onSelectSite={setSelectedSite} />
      )}

      <CreateSiteModal
        isOpen={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
        onSubmit={handleCreateSite}
      />
    </div>
  );
};

export default WebsitesView;
