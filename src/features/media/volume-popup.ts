export const VOLUME_POPUP_LABEL = 'volume-popup'
export const VOLUME_POPUP_OPEN_EVENT = 'volume://popup-open'
export const VOLUME_POPUP_CLOSE_EVENT = 'volume://popup-close'
export const VOLUME_POPUP_HOVER_CHANGED_EVENT = 'volume://popup-hover-changed'
export const VOLUME_POPUP_TRANSITION_MS = 160
export const VOLUME_POPUP_HOVER_BRIDGE_MS = 90

export interface VolumePopupOwnerPayload {
  ownerLabel: string
}

export interface VolumePopupOpenPayload extends VolumePopupOwnerPayload {
  themeColor: string
  generation: number
}

export interface VolumePopupHoverPayload extends VolumePopupOwnerPayload {
  hovered: boolean
}
