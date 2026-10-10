import { IS_MOBILE } from "@pp/ui";
import Desktop from "./DesktopLogs";
import Mobile from "./MobileLogs";

export default function Page() {
  return IS_MOBILE ? <Mobile /> : <Desktop />;
}
