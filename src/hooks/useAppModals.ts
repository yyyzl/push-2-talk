import { useCallback, useState } from "react";
import type { ServiceModalTab } from "../types";

export type UseAppModalsResult = {
  showServiceModal: boolean;
  setShowServiceModal: React.Dispatch<React.SetStateAction<boolean>>;
  serviceModalTab: ServiceModalTab;
  setServiceModalTab: React.Dispatch<React.SetStateAction<ServiceModalTab>>;

  showSettingsModal: boolean;
  openSettingsModal: () => void;
  closeSettingsModal: () => void;

  showApiKey: boolean;
  setShowApiKey: React.Dispatch<React.SetStateAction<boolean>>;

  openServiceModal: (tab: ServiceModalTab) => void;
  closeServiceModal: () => void;
  openDictionaryModal: () => void;
};

export function useAppModals(): UseAppModalsResult {
  const [showServiceModal, setShowServiceModal] = useState(false);
  const [serviceModalTab, setServiceModalTab] = useState<ServiceModalTab>("asr");
  const [showSettingsModal, setShowSettingsModal] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);

  const openServiceModal = useCallback((tab: ServiceModalTab) => {
    setServiceModalTab(tab);
    setShowServiceModal(true);
  }, []);

  const closeServiceModal = useCallback(() => {
    setShowServiceModal(false);
  }, []);

  const openDictionaryModal = useCallback(() => {
    openServiceModal("dictionary");
  }, [openServiceModal]);

  const openSettingsModal = useCallback(() => {
    setShowSettingsModal(true);
  }, []);

  const closeSettingsModal = useCallback(() => {
    setShowSettingsModal(false);
  }, []);

  return {
    showServiceModal,
    setShowServiceModal,
    serviceModalTab,
    setServiceModalTab,
    showSettingsModal,
    openSettingsModal,
    closeSettingsModal,
    showApiKey,
    setShowApiKey,
    openServiceModal,
    closeServiceModal,
    openDictionaryModal,
  };
}

