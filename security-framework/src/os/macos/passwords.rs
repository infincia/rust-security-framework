//! Password support.

use security_framework_sys::keychain::*;
use security_framework_sys::base::{SecKeychainRef, errSecSuccess};
use security_framework_sys::keychain_item::{SecKeychainItemDelete,
                                            SecKeychainItemModifyAttributesAndData};
use core_foundation_sys::base::{CFTypeRef, CFRelease};
use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use keychain::SecKeychain;
use std::ptr;
use std::ffi::CString;
use libc::c_void;

use cvt;
use base::Result;

/// Find a generic password in the default list of keychains.
///
/// The underlying system supports passwords with 0 values, so this
/// returns a vector of bytes rather than a string.
pub fn find_generic_password(keychains: Option<&[SecKeychain]>,
                             service: &str, account: &str) -> Result<Vec<u8>> {

    let keychain_or_array = match keychains {
        None => ptr::null(),
        Some(refs) if refs.len() == 1 => refs[0].as_CFTypeRef(),
        Some(refs) => CFArray::from_CFTypes(refs).as_CFTypeRef(),
    };

    let service_name_len = service.len() as u32;
    let service_name = CString::new(service).unwrap();

    let account_name_len = account.len() as u32;
    let account_name = CString::new(account).unwrap();

    let mut raw_len = 0;
    let mut raw = ptr::null_mut();

    unsafe {
        try!(cvt(SecKeychainFindGenericPassword(keychain_or_array,
                                                service_name_len,
                                                service_name.as_ptr(),
                                                account_name_len,
                                                account_name.as_ptr(),
                                                &mut raw_len,
                                                &mut raw,
                                                &mut ptr::null_mut())));

        // Copy the returned password.
        // https://doc.rust-lang.org/std/ptr/fn.copy.html
        let len = raw_len as usize;
        let mut password = Vec::with_capacity(len);
        password.set_len(len);
        ptr::copy(raw, password.as_mut_ptr(), len);

        // Now free the password.
        try!(cvt(SecKeychainItemFreeContent(ptr::null(),
                                            raw as *const c_void)));

        Ok(password)
    }
}

/// Set a generic password in the default keychain.
pub fn set_generic_password(keychain_opt: Option<&SecKeychain>,
                            service: &str, account: &str, password: &[u8])
                            -> Result<()> {

    let keychain_ref = match keychain_opt {
        None => ptr::null(),
        Some(keychain) => keychain.as_CFTypeRef(),
    };

    let service_name_len = service.len() as u32;
    let service_name = CString::new(service).unwrap();

    let account_name_len = account.len() as u32;
    let account_name = CString::new(account).unwrap();

    let password_len = password.len() as u32;
    let mut item = ptr::null_mut();

    unsafe {
        let status = SecKeychainFindGenericPassword(keychain_ref,
                                                    service_name_len,
                                                    service_name.as_ptr(),
                                                    account_name_len,
                                                    account_name.as_ptr(),
                                                    ptr::null_mut(),
                                                    ptr::null_mut(),
                                                    &mut item);

        match status {
            errSecSuccess => {
                try!(cvt(SecKeychainItemModifyAttributesAndData(
                    item, ptr::null(), password_len, password.as_ptr())));
            },
            _ => {
                try!(cvt(
                    SecKeychainAddGenericPassword(
                        keychain_ref as SecKeychainRef,
                        service_name_len,
                        service_name.as_ptr(),
                        account_name_len,
                        account_name.as_ptr(),
                        password_len,
                        password.as_ptr(),
                        ptr::null_mut())
                        ));
            }
        }
    }

    Ok(())
}

/// Delete a generic password.
pub fn delete_generic_password(keychains: Option<&[SecKeychain]>,
                               service: &str, account: &str) -> Result<()> {

    let keychain_or_array = match keychains {
        None => ptr::null(),
        Some(refs) if refs.len() == 1 => refs[0].as_CFTypeRef(),
        Some(refs) => CFArray::from_CFTypes(refs).as_CFTypeRef(),
    };

    let service_name_len = service.len() as u32;
    let service_name = CString::new(service).unwrap();

    let account_name_len = account.len() as u32;
    let account_name = CString::new(account).unwrap();

    let mut item = ptr::null_mut();

    unsafe {
         try!(cvt(SecKeychainFindGenericPassword(keychain_or_array,
                                                service_name_len,
                                                service_name.as_ptr(),
                                                account_name_len,
                                                account_name.as_ptr(),
                                                ptr::null_mut(),
                                                ptr::null_mut(),
                                                &mut item)));

        SecKeychainItemDelete(item);
        CFRelease(item as CFTypeRef);
        Ok(())
    }
}


#[cfg(test)]
mod test {
    use tempdir::TempDir;
    use keychain::{CreateOptions, SecKeychain};

    use super::*;

    fn temp_keychain_setup(name: &str) -> (TempDir, SecKeychain) {
        let dir = TempDir::new("passwords").unwrap();
        let keychain = CreateOptions::new()
            .password("foobar")
            .create(dir.path().join(name.to_string() + ".keychain"))
            .unwrap();

        (dir, keychain)
    }

    fn temp_keychain_teardown(dir: TempDir) -> () {
        dir.close().unwrap();
    }

    #[test]
    fn missing_password_temp() {
        let (dir, keychain) = temp_keychain_setup("missing_password");
        let keychains = vec![keychain];

        let service = "temp_this_service_does_not_exist";
        let account = "this_account_is_bogus";
        let password = find_generic_password(Some(&keychains),
                                             service, account);

        assert!(password.is_err());

        temp_keychain_teardown(dir);
    }

    #[test]
    #[cfg(feature = "default_keychain_tests")]
    fn missing_password_default() {
        let service = "default_this_service_does_not_exist";
        let account = "this_account_is_bogus";
        let password = find_generic_password(None, service, account);

        assert!(password.is_err());
    }

    #[test]
    fn round_trip_password_temp() {
        let (dir, keychain) = temp_keychain_setup("round_trip_password");
        let keychains = vec![keychain];

        let service = "test_round_trip_password_temp";
        let account = "temp_this_is_the_test_account";
        let password = String::from("deadbeef").into_bytes();

        set_generic_password(Some(&keychains[0]),
                             service, account, &password).unwrap();
        let found = find_generic_password(Some(&keychains),
                                          service, account).unwrap();
        assert_eq!(found, password);

        delete_generic_password(Some(&keychains), service, account).unwrap();

        temp_keychain_teardown(dir);
    }

    #[test]
    #[cfg(feature = "default_keychain_tests")]
    fn round_trip_password_default() {
        let service = "test_round_trip_password_default";
        let account = "this_is_the_test_account";
        let password = String::from("deadbeef").into_bytes();

        set_generic_password(None, service, account, &password).unwrap();
        let found = find_generic_password(None, service, account).unwrap();
        assert_eq!(found, password);

        delete_generic_password(None, service, account).unwrap();
    }

    #[test]
    fn change_password_temp() {
        let (dir, keychain) = temp_keychain_setup("change_password");
        let keychains = vec![keychain];

        let service = "test_change_password_temp";
        let account = "this_is_the_test_account";
        let pw1 = String::from("password1").into_bytes();
        let pw2 = String::from("password2").into_bytes();

        set_generic_password(Some(&keychains[0]), service, account, &pw1)
            .expect("set_generic_password");
        let found = find_generic_password(Some(&keychains),
                                          service, account)
            .expect("find_generic_password");
        assert_eq!(found, pw1);

        set_generic_password(Some(&keychains[0]), service, account, &pw2)
            .expect("set_generic_password2");
        let found = find_generic_password(Some(&keychains),
                                               service, account)
            .expect("find_generic_password2");
        assert_eq!(found, pw2);

        delete_generic_password(Some(&keychains), service, account)
            .expect("delete_generic_password");

        temp_keychain_teardown(dir);
    }

    #[test]
    #[cfg(feature = "default_keychain_tests")]
    fn change_password_default() {
        let service = "test_change_password_default";
        let account = "this_is_the_test_account";
        let pw1 = String::from("password1").into_bytes();
        let pw2 = String::from("password2").into_bytes();

        set_generic_password(None, service, account, &pw1).unwrap();
        let found = find_generic_password(None, service, account).unwrap();
        assert_eq!(found, pw1);

        set_generic_password(None, service, account, &pw2).unwrap();
        let found = find_generic_password(None, service, account).unwrap();
        assert_eq!(found, pw2);

        delete_generic_password(None, service, account).unwrap();
    }
}