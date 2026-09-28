import { useState } from "react";
import { createRoot } from "react-dom/client";
import { Select } from "../../src/components/common/Select";
import "../../src/index.css";

function Fixture() {
  const [value, setValue] = useState("saved-removed");
  const [parentClicks, setParentClicks] = useState(0);
  return <main className="p-8 space-y-5 font-sans">
    <div onClick={() => setParentClicks(n => n + 1)} style={{ width: 220, height: 46, overflow: "hidden" }}>
      <Select aria-label="模型选择" value={value} onChange={setValue} options={[
        { value: "", label: "跟随默认" },
        { value: "disabled", label: "不可选旧模型", disabled: true },
        { value: "alpha", label: "Alpha" },
        { value: "long", label: "很长的模型名称，用于检查窗口边缘和多行换行显示，不能超出屏幕" },
        ...Array.from({ length: 30 }, (_, index) => ({ value: `model-${index}`, label: `Model ${index}` })),
      ]} />
    </div>
    <output aria-label="当前值">{JSON.stringify(value)}</output>
    <output aria-label="父项点击次数">{parentClicks}</output>
    <Select aria-label="空列表" value="" onChange={() => { throw new Error("empty select changed"); }} options={[]} />
    <button>其他控件</button>
  </main>;
}
createRoot(document.getElementById("root")!).render(<Fixture />);
