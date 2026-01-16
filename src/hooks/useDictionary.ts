import type React from "react";
import { useEffect, useRef, useState } from "react";

const DICTIONARY_STORAGE_KEY = "pushtotalk_dictionary";

export type UseDictionaryResult = {
  dictionary: string[];
  setDictionary: React.Dispatch<React.SetStateAction<string[]>>;

  newWord: string;
  setNewWord: React.Dispatch<React.SetStateAction<string>>;

  duplicateHint: boolean;
  setDuplicateHint: React.Dispatch<React.SetStateAction<boolean>>;

  editingIndex: number | null;
  editingValue: string;
  setEditingValue: React.Dispatch<React.SetStateAction<string>>;

  handleAddWord: () => void;
  handleDeleteWord: (index: number) => void;
  handleStartEdit: (index: number) => void;
  handleSaveEdit: () => void;
  handleCancelEdit: () => void;
};

export function useDictionary(initialDictionary: string[] = []): UseDictionaryResult {
  const [dictionary, setDictionary] = useState<string[]>(() => {
    try {
      const saved = localStorage.getItem(DICTIONARY_STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (Array.isArray(parsed)) return parsed.filter((w) => typeof w === "string");
      }
    } catch {
      // ignore
    }
    return initialDictionary;
  });
  const [newWord, setNewWord] = useState("");
  const [duplicateHint, setDuplicateHint] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [editingValue, setEditingValue] = useState("");
  const duplicateHintTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (duplicateHintTimeoutRef.current) {
        window.clearTimeout(duplicateHintTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    try {
      localStorage.setItem(DICTIONARY_STORAGE_KEY, JSON.stringify(dictionary));
    } catch {
      // ignore
    }
  }, [dictionary]);

  const showDuplicateHint = () => {
    setDuplicateHint(true);
    if (duplicateHintTimeoutRef.current) {
      window.clearTimeout(duplicateHintTimeoutRef.current);
    }
    duplicateHintTimeoutRef.current = window.setTimeout(() => {
      setDuplicateHint(false);
    }, 2000);
  };

  const handleAddWord = () => {
    const word = newWord.trim();
    if (!word) return;

    if (dictionary.includes(word)) {
      showDuplicateHint();
      return;
    }

    setDictionary((prev) => [...prev, word]);
    setNewWord("");
  };

  const handleDeleteWord = (index: number) => {
    setDictionary((prev) => prev.filter((_, i) => i !== index));

    if (editingIndex === index) {
      setEditingIndex(null);
      setEditingValue("");
    } else if (editingIndex !== null && index < editingIndex) {
      setEditingIndex(editingIndex - 1);
    }
  };

  const handleStartEdit = (index: number) => {
    setEditingIndex(index);
    setEditingValue(dictionary[index] || "");
  };

  const handleSaveEdit = () => {
    if (editingIndex === null) return;

    const word = editingValue.trim();
    const isDuplicate = dictionary.some((w, i) => i !== editingIndex && w === word);
    if (isDuplicate) {
      showDuplicateHint();
      return;
    }

    if (word) {
      setDictionary((prev) => prev.map((w, i) => (i === editingIndex ? word : w)));
    }

    setEditingIndex(null);
    setEditingValue("");
  };

  const handleCancelEdit = () => {
    setEditingIndex(null);
    setEditingValue("");
  };

  return {
    dictionary,
    setDictionary,
    newWord,
    setNewWord,
    duplicateHint,
    setDuplicateHint,
    editingIndex,
    editingValue,
    setEditingValue,
    handleAddWord,
    handleDeleteWord,
    handleStartEdit,
    handleSaveEdit,
    handleCancelEdit,
  };
}
