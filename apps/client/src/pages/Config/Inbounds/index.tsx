import { IS_MOBILE } from "@pp/ui";
import Desktop from "./DesktopInbounds";
import Mobile from "./MobileInbounds";

export default function Page() {
  return IS_MOBILE ? <Mobile /> : <Desktop />;
}
