/**
 * 共享线性图标（Heroicons 24/outline 路径内联，无运行时依赖）。
 * 供 T2 组件（Modal/AlertDialog/Select）与页面复用；Alert 的历史内联路径逐步收敛到这里。
 */
interface IconProps {
  className?: string;
}

function Svg({ className, path }: IconProps & { path: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      <path d={path} />
    </svg>
  );
}

export function XMarkIcon({ className }: IconProps) {
  return <Svg className={className} path="M6 18 18 6M6 6l12 12" />;
}

export function ChevronUpDownIcon({ className }: IconProps) {
  return <Svg className={className} path="M8.25 15 12 18.75 15.75 15m-7.5-6L12 5.25 15.75 9" />;
}

export function CheckIcon({ className }: IconProps) {
  return <Svg className={className} path="m4.5 12.75 6 6 9-13.5" />;
}

export function ExclamationTriangleIcon({ className }: IconProps) {
  return (
    <Svg
      className={className}
      path="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z"
    />
  );
}

export function InformationCircleIcon({ className }: IconProps) {
  return (
    <Svg
      className={className}
      path="M11.25 11.25l.041-.02a.75.75 0 0 1 1.063.852l-.708 2.836a.75.75 0 0 0 1.063.853l.041-.021M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9-3.75h.008v.008H12V8.25Z"
    />
  );
}

export function CheckCircleIcon({ className }: IconProps) {
  return <Svg className={className} path="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />;
}
