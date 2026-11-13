import isMobileLib from 'is-mobile'

let _isAndroid = false

if (typeof navigator !== 'undefined') {
  _isAndroid =
    isMobileLib() ||
    /android/i.test(navigator.userAgent) ||
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform === 'Android'
}

export function useUserAgent() {
  return { isAndroid: _isAndroid }
}
