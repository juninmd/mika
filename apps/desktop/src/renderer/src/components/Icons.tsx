import type { LucideProps } from 'lucide-react'
import * as L from 'lucide-react'
import type { ComponentType, SVGProps } from 'react'

type IconProps = SVGProps<SVGSVGElement> & { size?: number | string }
type NamedIconComponent = ComponentType<IconProps>

function wrap(Icon: ComponentType<LucideProps>): NamedIconComponent {
  return Icon as unknown as NamedIconComponent
}

export const BrainIcon = wrap(L.Brain)
export const MicrophoneIcon = wrap(L.Mic)
export const MessageIcon = wrap(L.MessageCircle)
export const HistoryIcon = wrap(L.History)
export const CloseIcon = wrap(L.X)
export const XIcon = CloseIcon
export const StopIcon = wrap(L.Square)
export const SpeakerIcon = wrap(L.Volume2)
export const SaveIcon = wrap(L.Save)
export const SearchIcon = wrap(L.Search)
export const SparkIcon = wrap(L.Sparkles)
export const CheckIcon = wrap(L.Check)
export const WrenchIcon = wrap(L.Wrench)
export const EyeIcon = wrap(L.Eye)
export const CpuIcon = wrap(L.Cpu)
const SettingsIcon = wrap(L.Sliders)
export const CopyIcon = wrap(L.Copy)
export const CoinsIcon = wrap(L.Coins)
const AlarmClockIcon = wrap(L.AlarmClock)
const ArchiveIcon = wrap(L.Archive)
const BookOpenIcon = wrap(L.BookOpen)
const CheckSquareIcon = wrap(L.CheckSquare)
const ClipboardIcon = wrap(L.Clipboard)
const ClockIcon = wrap(L.Clock)
export const CloudIcon = wrap(L.Cloud)
const CodeIcon = wrap(L.Code)
const FileTextIcon = wrap(L.FileText)
const FolderOpenIcon = wrap(L.FolderOpen)
const Gamepad2Icon = wrap(L.Gamepad2)
function GithubIcon(props: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.85"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...props}
    >
      <path d="M15 21v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 19 4.77 5.07 5.07 0 0 0 18.91 1S17.73.65 15 2.48a13.38 13.38 0 0 0-6 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V21" />
      <path d="M9 18c-4.5 1.4-4.5-2.5-6-3" />
    </svg>
  )
}
export const GlobeIcon = wrap(L.Globe)
const GraduationCapIcon = wrap(L.GraduationCap)
const HeartIcon = wrap(L.Heart)
const HomeIcon = wrap(L.Home)
const MonitorIcon = wrap(L.Monitor)
const MoonIcon = wrap(L.Moon)
const SunIcon = wrap(L.Sun)
const MouseCursorIcon = wrap(L.MousePointer)
const MouseClickIcon = wrap(L.MousePointerClick)
const MusicNoteIcon = wrap(L.Music)
const NewspaperIcon = wrap(L.Newspaper)
const RssIcon = wrap(L.Rss)
export const SendIcon = wrap(L.Send)
const ServerIcon = wrap(L.Server)
const TerminalIcon = wrap(L.Terminal)
const TimerIcon = wrap(L.Timer)
const TrendingUpIcon = wrap(L.TrendingUp)
export const UserIcon = wrap(L.User)
const UsersIcon = wrap(L.Users)
const VideoIcon = wrap(L.Video)
const WebhookIcon = wrap(L.Webhook)
const PlayIcon = wrap(L.Play)
const PlugIcon = wrap(L.Plug)
const ShieldIcon = wrap(L.Shield)
export const PaletteIcon = wrap(L.Palette)
export const KeyboardIcon = wrap(L.Keyboard)

const NAMED_ICON_MAP: Record<string, NamedIconComponent> = {
  Palette: PaletteIcon,
  AlarmClock: AlarmClockIcon,
  Archive: ArchiveIcon,
  BookOpen: BookOpenIcon,
  Brain: BrainIcon,
  CheckSquare: CheckSquareIcon,
  Clipboard: ClipboardIcon,
  Clock: ClockIcon,
  Cloud: CloudIcon,
  Code: CodeIcon,
  Cpu: CpuIcon,
  Eye: EyeIcon,
  FileText: FileTextIcon,
  FolderOpen: FolderOpenIcon,
  Gamepad2: Gamepad2Icon,
  Github: GithubIcon,
  Globe: GlobeIcon,
  GraduationCap: GraduationCapIcon,
  Heart: HeartIcon,
  Home: HomeIcon,
  MessageCircle: MessageIcon,
  Mic: MicrophoneIcon,
  Monitor: MonitorIcon,
  MonitorCog: SettingsIcon,
  Moon: MoonIcon,
  Mouse: MouseCursorIcon,
  MousePointerClick: MouseClickIcon,
  Music: MusicNoteIcon,
  Newspaper: NewspaperIcon,
  Play: PlayIcon,
  Plug: PlugIcon,
  Rss: RssIcon,
  Send: SendIcon,
  Server: ServerIcon,
  Shield: ShieldIcon,
  Spark: SparkIcon,
  Star: SparkIcon,
  Sun: SunIcon,
  Terminal: TerminalIcon,
  Timer: TimerIcon,
  TrendingUp: TrendingUpIcon,
  Tv: MonitorIcon,
  User: UserIcon,
  Users: UsersIcon,
  Volume2: SpeakerIcon,
  Video: VideoIcon,
  Webhook: WebhookIcon,
  Wrench: WrenchIcon,
  Zap: SparkIcon,
}

export function isNamedIcon(name?: string): boolean {
  return Boolean(name && name in NAMED_ICON_MAP)
}

export function resolveNamedIcon(name?: string): NamedIconComponent {
  if (!name) return SparkIcon
  return NAMED_ICON_MAP[name] ?? SparkIcon
}
