/**
 * @pp/ui —— 客户端自研组件库（ADR-0013）。
 * Base UI headless 底座 + Tailwind v4 语义令牌（`@pp/ui/tokens.css`），
 * 编译期平台分发（`IS_MOBILE`，vite define 注入）。
 */

export { IS_MOBILE } from "./platform.ts";
export { cx } from "./utils.ts";

// T0 纯样式
export { Alert, type AlertStatus } from "./components/Alert.tsx";
export { Card } from "./components/Card.tsx";
export { Chip, type ChipColor } from "./components/Chip.tsx";
export { Label } from "./components/Label.tsx";
export { Separator } from "./components/Separator.tsx";
export { Spinner } from "./components/Spinner.tsx";

// T1 headless 封装
export { Button, type ButtonSize, type ButtonVariant } from "./components/Button.tsx";
export { Field } from "./components/Field.tsx";
export { Input, inputClassName, textareaClassName } from "./components/Input.tsx";
export { Switch } from "./components/Switch.tsx";

// T2 Overlay / Select 族
export {
  AlertDialog,
  ConfirmDialog,
  type AlertDialogStatus,
  type ConfirmDialogProps,
} from "./components/AlertDialog.tsx";
export { BottomSheet, Modal, type BottomSheetProps, type ModalBackdropProps } from "./components/Modal.tsx";
export { ListBox, Select, type ListBoxItemProps, type SelectRootProps } from "./components/Select.tsx";

// 共享图标（Heroicons outline 内联）
export {
  CheckCircleIcon,
  CheckIcon,
  ChevronUpDownIcon,
  ExclamationTriangleIcon,
  InformationCircleIcon,
  XMarkIcon,
} from "./icons.tsx";

export { Checkbox } from "./components/Checkbox";
export { Radio, RadioGroup } from "./components/Radio";
export { Tabs } from "./components/Tabs";
export { Table } from "./components/Table";
export { Avatar } from "./components/Avatar";
export { Meter } from "./components/Meter";
export { TextArea } from "./components/Input";

export { Segmented, SegmentedButton, Navbar, Fab } from "./components/Navigation";

export { ToastRegion, type ToastItem } from "./components/Toast";

export { SelectField, type SelectOption } from "./components/SelectField";
export { DataList, type DataListColumn, type DataListProps } from "./components/DataList";

export { Shell } from "./components/Shell";

export { InlineAlert, type InlineAlertKind } from "./components/InlineAlert";
