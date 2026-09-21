export const VOLUME_POPUP_LABEL = 'volume-popup'
export const VOLUME_POPUP_OPEN_EVENT = 'volume://popup-open'
export const VOLUME_POPUP_CLOSE_EVENT = 'volume://popup-close'
export const VOLUME_POPUP_HOVER_CHANGED_EVENT = 'volume://popup-hover-changed'
/**
 * 离场动画时长：同时作为 CSS 过渡时长（见 `pages/volume`）和隐藏原生窗的延迟，
 * 两者必须相等，否则动画会被截断或窗口在动画结束后仍停留。
 */
export const VOLUME_POPUP_TRANSITION_MS = 160
/**
 * 跨原生窗口 hover 的补偿窗口：bar 按钮与悬浮窗是两个原生窗口，指针在两者间移动时
 * 都会先收到一次离开事件，延迟这么久再判定关闭才能把它们连成一片 hover 区域。
 */
export const VOLUME_POPUP_HOVER_BRIDGE_MS = 90

export interface VolumePopupOwnerPayload {
  ownerLabel: string
}

export interface VolumePopupOpenPayload extends VolumePopupOwnerPayload {
  themeColor: string
  /** 显示代次：由原生侧在每次显示时分配，隐藏时必须原样回传，否则过期请求会被原生丢弃。 */
  generation: number
}

export interface VolumePopupHoverPayload extends VolumePopupOwnerPayload {
  hovered: boolean
}
