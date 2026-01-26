import type React from "react";
import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DictionaryEntry } from "../types";
import { parseEntry } from "../utils/dictionaryUtils";

export type UseDictionaryResult = {
  dictionary: DictionaryEntry[];
  setDictionary: React.Dispatch<React.SetStateAction<DictionaryEntry[]>>;

  newWord: string;
  setNewWord: React.Dispatch<React.SetStateAction<string>>;

  duplicateHint: boolean;
  setDuplicateHint: React.Dispatch<React.SetStateAction<boolean>>;

  editingIndex: number | null;
  editingValue: string;
  setEditingValue: React.Dispatch<React.SetStateAction<string>>;

  handleAddWord: () => void;
  handleDeleteWord: (word: string) => void;
  handleStartEdit: (index: number) => void;
  handleSaveEdit: () => void;
  handleCancelEdit: () => void;
  handleBatchDelete: (words: string[]) => void;

  refreshDictionary: () => Promise<void>;
};

export function useDictionary(initialDictionary: string[] = []): UseDictionaryResult {
  const [dictionary, setDictionary] = useState<DictionaryEntry[]>(
    initialDictionary.map(parseEntry)
  );
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

  const showDuplicateHint = () => {
    setDuplicateHint(true);
    if (duplicateHintTimeoutRef.current) {
      window.clearTimeout(duplicateHintTimeoutRef.current);
    }
    duplicateHintTimeoutRef.current = window.setTimeout(() => {
      setDuplicateHint(false);
    }, 2000);
  };

  // 刷新词典
  const refreshDictionary = useCallback(async () => {
    try {
      const entries = await invoke<string[]>("get_dictionary_entries");
      setDictionary(entries.map(parseEntry));
    } catch (error) {
      console.error("刷新词典失败:", error);
    }
  }, []);

  // 监听词典更新事件（实时刷新）
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await listen("dictionary_updated", () => {
        console.log("收到词典更新事件，刷新词典");
        refreshDictionary();
      });
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [refreshDictionary]);

  // 添加词汇
  const handleAddWord = useCallback(async () => {
    const word = newWord.trim();
    if (!word) return;

    // 检查是否已存在
    if (dictionary.some((e) => e.word === word)) {
      showDuplicateHint();
      return;
    }

    try {
      await invoke("add_learned_word", { word, source: "manual" });
      setNewWord("");
      // 不需要手动刷新，事件监听会自动刷新
    } catch (error) {
      console.error("添加词汇失败:", error);
    }
  }, [newWord, dictionary]);

  // 删除词汇
  const handleDeleteWord = useCallback(async (word: string) => {
    try {
      await invoke("delete_dictionary_entries", { words: [word] });
      // 不需要手动刷新，事件监听会自动刷新

      // 如果正在编辑被删除的词汇，取消编辑
      if (editingIndex !== null) {
        const deletedIndex = dictionary.findIndex((e) => e.word === word);
        if (deletedIndex === editingIndex) {
          setEditingIndex(null);
          setEditingValue("");
        } else if (deletedIndex < editingIndex) {
          setEditingIndex(editingIndex - 1);
        }
      }
    } catch (error) {
      console.error("删除词汇失败:", error);
    }
  }, [dictionary, editingIndex]);

  // 批量删除
  const handleBatchDelete = useCallback(async (words: string[]) => {
    if (words.length === 0) return;

    try {
      await invoke("delete_dictionary_entries", { words });
      // 不需要手动刷新，事件监听会自动刷新
      setEditingIndex(null);
      setEditingValue("");
    } catch (error) {
      console.error("批量删除失败:", error);
    }
  }, []);

  // 开始编辑
  const handleStartEdit = useCallback((index: number) => {
    setEditingIndex(index);
    setEditingValue(dictionary[index]?.word || "");
  }, [dictionary]);

  // 保存编辑
  const handleSaveEdit = useCallback(async () => {
    if (editingIndex === null) return;

    const word = editingValue.trim();
    const currentEntry = dictionary[editingIndex];
    if (!currentEntry) return;

    // 检查是否重复
    const isDuplicate = dictionary.some((e, i) => i !== editingIndex && e.word === word);
    if (isDuplicate) {
      showDuplicateHint();
      return;
    }

    if (word && word !== currentEntry.word) {
      try {
        // 删除旧词条，添加新词条（保持来源）
        await invoke("delete_dictionary_entries", { words: [currentEntry.word] });
        await invoke("add_learned_word", { word, source: currentEntry.source });
        // 不需要手动刷新，事件监听会自动刷新
      } catch (error) {
        console.error("更新词汇失败:", error);
      }
    }

    setEditingIndex(null);
    setEditingValue("");
  }, [editingIndex, editingValue, dictionary]);

  // 取消编辑
  const handleCancelEdit = useCallback(() => {
    setEditingIndex(null);
    setEditingValue("");
  }, []);

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
    handleBatchDelete,
    refreshDictionary,
  };
}
