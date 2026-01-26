import { useEffect, useState, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { VocabularyLearningToast } from "../components/learning/VocabularyLearningToast";
import type { VocabularyLearningSuggestion } from "../types";

// 最大通知数量
const MAX_NOTIFICATIONS = 3;

export default function NotificationWindow() {
  const [suggestions, setSuggestions] = useState<VocabularyLearningSuggestion[]>([]);

  // 获取当前窗口引用（更可靠的方式）
  const notificationWindow = getCurrentWindow();

  // 添加新建议
  const addSuggestion = useCallback((suggestion: VocabularyLearningSuggestion) => {
    setSuggestions((prev) => {
      // 去重：如果已存在相同词汇的建议，不重复添加
      if (prev.some((s) => s.word === suggestion.word)) {
        console.log("词汇已存在，跳过:", suggestion.word);
        return prev;
      }
      // 限制最大数量，移除最旧的
      const newList = [...prev, suggestion];
      if (newList.length > MAX_NOTIFICATIONS) {
        return newList.slice(-MAX_NOTIFICATIONS);
      }
      return newList;
    });
  }, []);

  // 移除建议
  const removeSuggestion = useCallback((id: string) => {
    setSuggestions((prev) => prev.filter((s) => s.id !== id));
  }, []);

  // 监听学习建议事件
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen<VocabularyLearningSuggestion>(
          "vocabulary_learning_suggestion",
          async (event) => {
            console.log("收到词库学习建议:", event.payload);
            addSuggestion(event.payload);

            // 调用后端命令显示窗口（后端处理定位，支持多显示器和高分屏）
            try {
              await invoke("show_notification_window");
            } catch (error) {
              console.error("显示通知窗口失败:", error);
            }
          }
        );
      } catch (error) {
        console.error("设置事件监听器失败:", error);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [addSuggestion]);

  // 当所有通知都消失时，隐藏窗口
  useEffect(() => {
    if (suggestions.length === 0) {
      notificationWindow.hide().catch(console.error);
    }
  }, [suggestions.length]);

  return (
    <div
      className="fixed inset-0 pointer-events-none"
      role="region"
      aria-live="polite"
      aria-label="词库学习通知区域"
    >
      {/* 通知堆叠容器 - 水平居中 */}
      <div className="fixed bottom-12 left-1/2 -translate-x-1/2 flex flex-col-reverse gap-3 pointer-events-auto">
        {suggestions.map((suggestion) => (
          <VocabularyLearningToast
            key={suggestion.id}
            suggestion={suggestion}
            onDismiss={() => removeSuggestion(suggestion.id)}
            onAdd={() => removeSuggestion(suggestion.id)}
          />
        ))}
      </div>
    </div>
  );
}
