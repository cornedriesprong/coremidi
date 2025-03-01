#![allow(clippy::unnecessary_cast)]

use core_foundation::base::{OSStatus, TCFType};
use core_foundation::string::{CFString, CFStringRef};

use coremidi_sys::{
    MIDIIOErrorNotification, MIDINotification, MIDIObjectAddRemoveNotification,
    MIDIObjectPropertyChangeNotification,
};

use crate::device::Device;
use crate::object::{Object, ObjectType};

#[derive(Debug, PartialEq)]
pub struct AddedRemovedInfo {
    pub parent: Object,
    pub parent_type: ObjectType,
    pub child: Object,
    pub child_type: ObjectType,
}

#[derive(Debug, PartialEq)]
pub struct PropertyChangedInfo {
    pub object: Object,
    pub object_type: ObjectType,
    pub property_name: String,
}

#[derive(Debug, PartialEq)]
pub struct IoErrorInfo {
    pub driver_device: Device,
    pub error_code: OSStatus,
}

/// A message describing a system state change.
/// See [MIDINotification](https://developer.apple.com/documentation/coremidi/midinotification).
///
#[derive(Debug, PartialEq)]
pub enum Notification {
    SetupChanged,
    ObjectAdded(AddedRemovedInfo),
    ObjectRemoved(AddedRemovedInfo),
    PropertyChanged(PropertyChangedInfo),
    ThruConnectionsChanged,
    SerialPortOwnerChanged,
    IoError(IoErrorInfo),
}

impl Notification {
    fn try_from_object_added_removed(
        notification: &MIDINotification,
    ) -> Result<Notification, OSStatus> {
        let add_remove_notification =
            unsafe { &*(notification as *const _ as *const MIDIObjectAddRemoveNotification) };
        let parent_type = ObjectType::try_from(add_remove_notification.parentType);
        let child_type = ObjectType::try_from(add_remove_notification.childType);
        match (parent_type, child_type) {
            (Ok(parent_type), Ok(child_type)) => {
                let add_remove_info = AddedRemovedInfo {
                    parent: Object(add_remove_notification.parent),
                    parent_type,
                    child: Object(add_remove_notification.child),
                    child_type,
                };
                match notification.messageID as ::std::os::raw::c_uint {
                    coremidi_sys::kMIDIMsgObjectAdded => {
                        Ok(Notification::ObjectAdded(add_remove_info))
                    }
                    coremidi_sys::kMIDIMsgObjectRemoved => {
                        Ok(Notification::ObjectRemoved(add_remove_info))
                    }
                    _ => unreachable!(),
                }
            }
            _ => Err(notification.messageID as OSStatus),
        }
    }

    fn try_from_property_changed(notification: &MIDINotification) -> Result<Notification, i32> {
        let property_changed_notification =
            unsafe { &*(notification as *const _ as *const MIDIObjectPropertyChangeNotification) };
        match ObjectType::try_from(property_changed_notification.objectType) {
            Ok(object_type) => {
                let property_name = {
                    let name_ref: CFStringRef = property_changed_notification.propertyName;
                    let name: CFString = unsafe { TCFType::wrap_under_get_rule(name_ref) };
                    name.to_string()
                };
                let property_changed_info = PropertyChangedInfo {
                    object: Object(property_changed_notification.object),
                    object_type,
                    property_name,
                };
                Ok(Notification::PropertyChanged(property_changed_info))
            }
            Err(_) => Err(notification.messageID as i32),
        }
    }

    fn from_io_error(notification: &MIDINotification) -> Notification {
        let io_error_notification =
            unsafe { &*(notification as *const _ as *const MIDIIOErrorNotification) };
        let io_error_info = IoErrorInfo {
            driver_device: Device {
                object: Object(io_error_notification.driverDevice),
            },
            error_code: io_error_notification.errorCode,
        };
        Notification::IoError(io_error_info)
    }
}

impl TryFrom<&MIDINotification> for Notification {
    type Error = OSStatus;

    fn try_from(notification: &MIDINotification) -> Result<Self, Self::Error> {
        match notification.messageID as ::std::os::raw::c_uint {
            coremidi_sys::kMIDIMsgSetupChanged => Ok(Notification::SetupChanged),
            coremidi_sys::kMIDIMsgObjectAdded | coremidi_sys::kMIDIMsgObjectRemoved => {
                Self::try_from_object_added_removed(notification)
            }
            coremidi_sys::kMIDIMsgPropertyChanged => Self::try_from_property_changed(notification),
            coremidi_sys::kMIDIMsgThruConnectionsChanged => {
                Ok(Notification::ThruConnectionsChanged)
            }
            coremidi_sys::kMIDIMsgSerialPortOwnerChanged => {
                Ok(Notification::SerialPortOwnerChanged)
            }
            coremidi_sys::kMIDIMsgIOError => Ok(Self::from_io_error(notification)),
            unknown => Err(unknown as OSStatus),
        }
    }
}

#[cfg(test)]
mod tests {

    use core_foundation::base::{OSStatus, TCFType};
    use core_foundation::string::CFString;

    use coremidi_sys::{
        MIDIIOErrorNotification, MIDINotification, MIDINotificationMessageID,
        MIDIObjectAddRemoveNotification, MIDIObjectPropertyChangeNotification, MIDIObjectRef,
    };

    use crate::device::Device;
    use crate::notifications::{AddedRemovedInfo, IoErrorInfo, Notification, PropertyChangedInfo};
    use crate::object::{Object, ObjectType};

    #[test]
    fn notification_from_error() {
        let notification_raw = MIDINotification {
            messageID: 0xffff as MIDINotificationMessageID,
            messageSize: 8,
        };

        let notification = Notification::try_from(&notification_raw);

        assert!(notification.is_err());
        assert_eq!(notification.err().unwrap(), 0xffff as i32);
    }

    #[test]
    fn notification_from_setup_changed() {
        let notification_raw = MIDINotification {
            messageID: coremidi_sys::kMIDIMsgSetupChanged as MIDINotificationMessageID,
            messageSize: 8,
        };

        let notification = Notification::try_from(&notification_raw);

        assert!(notification.is_ok());
        assert_eq!(notification.unwrap(), Notification::SetupChanged);
    }

    #[test]
    fn notification_from_object_added() {
        let notification_raw = MIDIObjectAddRemoveNotification {
            messageID: coremidi_sys::kMIDIMsgObjectAdded as MIDINotificationMessageID,
            messageSize: 24,
            parent: 1 as MIDIObjectRef,
            parentType: coremidi_sys::kMIDIObjectType_Device,
            child: 2 as MIDIObjectRef,
            childType: coremidi_sys::kMIDIObjectType_Other,
        };

        let notification = Notification::try_from(unsafe {
            &*(&notification_raw as *const _ as *const MIDINotification)
        });

        assert!(notification.is_ok());

        let info = AddedRemovedInfo {
            parent: Object(1),
            parent_type: ObjectType::Device,
            child: Object(2),
            child_type: ObjectType::Other,
        };

        assert_eq!(notification.unwrap(), Notification::ObjectAdded(info));
    }

    #[test]
    fn notification_from_object_removed() {
        let notification_raw = MIDIObjectAddRemoveNotification {
            messageID: coremidi_sys::kMIDIMsgObjectRemoved as MIDINotificationMessageID,
            messageSize: 24,
            parent: 1 as MIDIObjectRef,
            parentType: coremidi_sys::kMIDIObjectType_Device,
            child: 2 as MIDIObjectRef,
            childType: coremidi_sys::kMIDIObjectType_Other,
        };

        let notification = Notification::try_from(unsafe {
            &*(&notification_raw as *const _ as *const MIDINotification)
        });

        assert!(notification.is_ok());

        let info = AddedRemovedInfo {
            parent: Object(1),
            parent_type: ObjectType::Device,
            child: Object(2),
            child_type: ObjectType::Other,
        };

        assert_eq!(notification.unwrap(), Notification::ObjectRemoved(info));
    }

    #[test]
    fn notification_from_object_added_removed_err() {
        let notification_raw = MIDIObjectAddRemoveNotification {
            messageID: coremidi_sys::kMIDIMsgObjectAdded as MIDINotificationMessageID,
            messageSize: 24,
            parent: 1 as MIDIObjectRef,
            parentType: coremidi_sys::kMIDIObjectType_Device,
            child: 2 as MIDIObjectRef,
            childType: 0xffff,
        };

        let notification = Notification::try_from(unsafe {
            &*(&notification_raw as *const _ as *const MIDINotification)
        });

        assert!(notification.is_err());
        assert_eq!(
            notification.err().unwrap(),
            coremidi_sys::kMIDIMsgObjectAdded as i32
        );

        let notification_raw = MIDIObjectAddRemoveNotification {
            messageID: coremidi_sys::kMIDIMsgObjectRemoved as MIDINotificationMessageID,
            messageSize: 24,
            parent: 1 as MIDIObjectRef,
            parentType: 0xffff,
            child: 2 as MIDIObjectRef,
            childType: coremidi_sys::kMIDIObjectType_Device,
        };

        let notification = Notification::try_from(unsafe {
            &*(&notification_raw as *const _ as *const MIDINotification)
        });

        assert!(notification.is_err());
        assert_eq!(
            notification.err().unwrap(),
            coremidi_sys::kMIDIMsgObjectRemoved as i32
        );
    }

    #[test]
    fn notification_from_property_changed() {
        let name = CFString::new("name");
        let notification_raw = MIDIObjectPropertyChangeNotification {
            messageID: coremidi_sys::kMIDIMsgPropertyChanged as MIDINotificationMessageID,
            messageSize: 24,
            object: 1 as MIDIObjectRef,
            objectType: coremidi_sys::kMIDIObjectType_Device,
            propertyName: name.as_concrete_TypeRef(),
        };

        let notification = Notification::try_from(unsafe {
            &*(&notification_raw as *const _ as *const MIDINotification)
        });

        assert!(notification.is_ok());

        let info = PropertyChangedInfo {
            object: Object(1),
            object_type: ObjectType::Device,
            property_name: "name".to_string(),
        };

        assert_eq!(notification.unwrap(), Notification::PropertyChanged(info));
    }

    #[test]
    fn notification_from_property_changed_error() {
        let name = CFString::new("name");
        let notification_raw = MIDIObjectPropertyChangeNotification {
            messageID: coremidi_sys::kMIDIMsgPropertyChanged as MIDINotificationMessageID,
            messageSize: 24,
            object: 1 as MIDIObjectRef,
            objectType: 0xffff,
            propertyName: name.as_concrete_TypeRef(),
        };

        let notification = Notification::try_from(unsafe {
            &*(&notification_raw as *const _ as *const MIDINotification)
        });

        assert!(notification.is_err());
        assert_eq!(
            notification.err().unwrap(),
            coremidi_sys::kMIDIMsgPropertyChanged as i32
        );
    }

    #[test]
    fn notification_from_thru_connections_changed() {
        let notification_raw = MIDINotification {
            messageID: coremidi_sys::kMIDIMsgThruConnectionsChanged as MIDINotificationMessageID,
            messageSize: 8,
        };

        let notification = Notification::try_from(&notification_raw);

        assert!(notification.is_ok());
        assert_eq!(notification.unwrap(), Notification::ThruConnectionsChanged);
    }

    #[test]
    fn notification_from_serial_port_owner_changed() {
        let notification_raw = MIDINotification {
            messageID: coremidi_sys::kMIDIMsgSerialPortOwnerChanged as MIDINotificationMessageID,
            messageSize: 8,
        };

        let notification = Notification::try_from(&notification_raw);

        assert!(notification.is_ok());
        assert_eq!(notification.unwrap(), Notification::SerialPortOwnerChanged);
    }

    #[test]
    fn notification_from_io_error() {
        let notification_raw = MIDIIOErrorNotification {
            messageID: coremidi_sys::kMIDIMsgIOError as MIDINotificationMessageID,
            messageSize: 16,
            driverDevice: 1 as MIDIObjectRef,
            errorCode: 123 as OSStatus,
        };

        let notification = Notification::try_from(unsafe {
            &*(&notification_raw as *const _ as *const MIDINotification)
        });

        assert!(notification.is_ok());

        let info = IoErrorInfo {
            driver_device: Device { object: Object(1) },
            error_code: 123 as OSStatus,
        };

        assert_eq!(notification.unwrap(), Notification::IoError(info));
    }
}
